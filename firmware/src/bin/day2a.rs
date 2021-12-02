// $ DEFMT_LOG=info cargo rb day2a
#![no_main]
#![no_std]

use firmware as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [USART1])]
mod app {
    use defmt::Format;
    use heapless::Vec;
    use postcard::{CobsAccumulator, FeedResult};
    use serde::{Deserialize, Serialize};
    use stm32f4xx_hal::{
        otg_fs::{UsbBus, UsbBusType, USB},
        prelude::*,
    };
    use usb_device::{bus::UsbBusAllocator, prelude::*};
    use usbd_serial::SerialPort;
    const BUF_SIZE: usize = 64;

    #[derive(Format, Serialize, Deserialize, Clone, Copy)]
    pub enum Direction {
        Up(u8),
        Down(u8),
        Forward(u8),
    }

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        usb_dev: UsbDevice<'static, UsbBus<USB>>,
        serial: SerialPort<'static, UsbBus<USB>>,
    }

    #[init(local = [ep_memory: [u32; 1024] = [0; 1024], usb_bus: Option<UsbBusAllocator<UsbBusType>> = None])]
    fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let rcc = ctx.device.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(100.mhz()).require_pll48clk().freeze();

        let gpioa = ctx.device.GPIOA.split();
        let usb = USB {
            usb_global: ctx.device.OTG_FS_GLOBAL,
            usb_device: ctx.device.OTG_FS_DEVICE,
            usb_pwrclk: ctx.device.OTG_FS_PWRCLK,
            pin_dm: gpioa.pa11.into_alternate(),
            pin_dp: gpioa.pa12.into_alternate(),
            hclk: clocks.hclk(),
        };
        let usb_bus = ctx.local.usb_bus;
        usb_bus.replace(UsbBus::new(usb, ctx.local.ep_memory));

        let serial = SerialPort::new(usb_bus.as_ref().unwrap());
        let usb_dev = UsbDeviceBuilder::new(usb_bus.as_ref().unwrap(), UsbVidPid(0x16c0, 0x27dd))
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")
            .device_class(usbd_serial::USB_CLASS_CDC)
            .build();

        defmt::info!("Send me some input data!");
        (Shared {}, Local { serial, usb_dev }, init::Monotonics())
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {}
    }

    #[task(binds=OTG_FS, local = [serial, usb_dev], priority = 3)]
    fn on_usb(ctx: on_usb::Context) {
        let serial = ctx.local.serial;
        if !ctx.local.usb_dev.poll(&mut [serial]) {
            return;
        }
        let mut buf = [0u8; BUF_SIZE];
        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                parser::spawn(Vec::from_slice(&buf[..count]).unwrap()).ok();
            }
            _ => {}
        }
    }

    #[task(local = [cobs_buf: CobsAccumulator<64> = CobsAccumulator::new(), x: i32 = 0, y: i32 = 0], priority = 2, capacity = 8)]
    fn parser(ctx: parser::Context, buf: Vec<u8, BUF_SIZE>) {
        let mut window = &buf[..];
        while !window.is_empty() {
            window = match ctx.local.cobs_buf.feed::<Direction>(window) {
                FeedResult::Consumed => break,
                FeedResult::OverFull(w) => w,
                FeedResult::DeserError(w) => w,
                FeedResult::Success { data, remaining } => {
                    let (x, y) = match data {
                        Direction::Up(d) => (*ctx.local.x, *ctx.local.y - d as i32),
                        Direction::Down(d) => (*ctx.local.x, *ctx.local.y + d as i32),
                        Direction::Forward(d) => (*ctx.local.x + d as i32, *ctx.local.y),
                    };
                    *ctx.local.x = x;
                    *ctx.local.y = y;
                    remaining
                }
            };
        }
        defmt::info!("Result: {}", *ctx.local.x * *ctx.local.y);
    }
}
