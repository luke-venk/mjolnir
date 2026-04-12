# Circle infractions UART messages

Arduino -> PC (USB Serial, 115200 baud), newline-delimited ASCII.

## Events
- CI,INFRACTION,<ms>
- CI,CLEAR,<ms>

## Optional
- CI,HEARTBEAT,<ms>
