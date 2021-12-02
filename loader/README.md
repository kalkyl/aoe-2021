# Advent of Code loaders

Companion apps for loading input data from a desktop computer into the MCU over USB serial.

## USB connection

| Signal | Color | Pin  | Nucleo    |
| ------ | ----- | ---- | --------- |
| V+     | Red   |      |           |
| D-     | White | PA11 | CN10 - 14 |
| D+     | Green | PA12 | CN10 - 12 |
| Gnd    | Black |      | CN10 - 9  |

D+ used for re-enumeration. You don't need to connect the V+ from the USB cable, as the NUCLEO is self powered.
