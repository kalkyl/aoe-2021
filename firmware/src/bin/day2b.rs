// $ DEFMT_LOG=info cargo rb day2b
#![no_main]
#![no_std]

use firmware as _; // global logger + panicking-behavior + memory layout

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [EXTI0, USART1])]
mod app {
    use defmt::Format;
    use heapless::Vec;
    use postcard::{CobsAccumulator, FeedResult};
    use serde::{Deserialize, Serialize};
    use ssd1306::{mode::BufferedGraphicsMode, prelude::*, I2CDisplayInterface, Ssd1306};
    use stm32f4xx_hal::{
        gpio::{Alternate, OpenDrain},
        i2c::I2c,
        otg_fs::{UsbBus, UsbBusType, USB},
        pac::I2C1,
        prelude::*,
        timer::{monotonic::fugit::ExtU32, monotonic::MonoTimer, Timer},
    };
    use usb_device::{bus::UsbBusAllocator, prelude::*};
    use usbd_serial::SerialPort;
    type SCL = stm32f4xx_hal::gpio::Pin<Alternate<OpenDrain, 4_u8>, 'B', 8_u8>;
    type SDA = stm32f4xx_hal::gpio::Pin<Alternate<OpenDrain, 4_u8>, 'B', 9_u8>;
    const BUF_SIZE: usize = 64;

    #[derive(Format, Serialize, Deserialize, Clone, Copy)]
    pub enum Direction {
        Up(u8),
        Down(u8),
        Forward(u8),
    }

    #[monotonic(binds = TIM2, default = true)]
    type MyMono = MonoTimer<stm32f4xx_hal::pac::TIM2, 1_000_000>;

    #[shared]
    struct Shared {
        display: Ssd1306<
            I2CInterface<I2c<I2C1, (SCL, SDA)>>,
            DisplaySize128x64,
            BufferedGraphicsMode<DisplaySize128x64>,
        >,
    }

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

        // Set up I2C.
        let gpiob = ctx.device.GPIOB.split();
        let scl = gpiob.pb8.into_alternate_open_drain();
        let sda = gpiob.pb9.into_alternate_open_drain();
        let i2c = I2c::new(ctx.device.I2C1, (scl, sda), 400.khz(), &clocks);

        // Configure the OLED display.
        let interface = I2CDisplayInterface::new(i2c);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init().unwrap();

        let mono = Timer::new(ctx.device.TIM2, &clocks).monotonic();
        update_display::spawn().ok();

        defmt::info!("Send me some input data!");
        (
            Shared { display },
            Local { serial, usb_dev },
            init::Monotonics(mono),
        )
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

    #[task(local = [cobs_buf: CobsAccumulator<64> = CobsAccumulator::new(), aim: i32 = 0, x: i32 = 0, y: i32 = 0], shared = [display], priority = 2, capacity = 8)]
    fn parser(mut ctx: parser::Context, buf: Vec<u8, BUF_SIZE>) {
        let mut window = &buf[..];
        while !window.is_empty() {
            window = match ctx.local.cobs_buf.feed::<Direction>(window) {
                FeedResult::Success { data, remaining } => {
                    let (aim, x, y) = match data {
                        Direction::Up(d) => (*ctx.local.aim - d as i32, *ctx.local.x, *ctx.local.y),
                        Direction::Down(d) => {
                            (*ctx.local.aim + d as i32, *ctx.local.x, *ctx.local.y)
                        }
                        Direction::Forward(d) => (
                            *ctx.local.aim,
                            *ctx.local.x + d as i32,
                            *ctx.local.y + *ctx.local.aim * (d as i32),
                        ),
                    };
                    *ctx.local.aim = aim;
                    *ctx.local.x = x;
                    *ctx.local.y = y;
                    ctx.shared.display.lock(|d| {
                        d.set_pixel(
                            ((x as f32 / 2_100.0) * 128.0) as _,
                            ((y as f32 / 1_000_000.0) * 64.0) as _,
                            true,
                        )
                    });
                    remaining
                }
                _ => break,
            };
        }
        defmt::info!("Result: {}", *ctx.local.x * *ctx.local.y);
    }

    #[task(shared = [display], priority = 2)]
    fn update_display(mut ctx: update_display::Context) {
        ctx.shared.display.lock(|d| d.flush().ok());
        update_display::spawn_after(15.millis()).ok();
    }
}
