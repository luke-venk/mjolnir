# Cameras
This project uses 2 [LUCID Vision Labs™ Atlas ATP124S](https://www.edmundoptics.com/p/lucid-vision-labst-atlas-atp124s-mc-sony-imx545-123mp-ip67-monochrome-camera/54295/) cameras with the following specifications:
- GigE Vision v2.0: global industrial camera interface standard that enables high-speed data transmission and device control over Gigabit Ethernet networks
- Resolution: 12.3 Megapixels
- Frame Rate: 42.5 frames per second
- Power over Ethernet
- Global shutter: required for our computer vision frames to be accurate
- Precise Time Synchronization (PTP): required for accurate calculations of where the object implemented
- Monochrome

We have 2 binaries related to using the cameras.
1. Discovery: Finding the cameras on the LAN
2. Recording: Recording usines the cameras on the LAN and writing the frames to disk

## Hardware Setup
1. Connect the switch to power (outlet if indoors, Jackery if outdoors). It will flash a green LED.
2. Connect the RJ45 to M12 connector cables to the switch (RJ45) and cameras (M12). They will flash red then eventually green LEDs.
3. Attach the lenses to the cameras
4. Connect the laptop to the switch using the USB-Ethernet Network Adapter

## Discover Cameras
This program is uses to discover cameras on the local area network (LAN).

To run this program, run the following command:  
`bazel run //backend:discover`  

An example output looks like the following:
```
TODO
```

Note that it takes up to a few minutes for the laptop, once plugged in, to be able to discover the cameras.

## Record from Cameras
To run the auxiliary binary for recording from the discovered cameras on the LAN, run the following command:
```
bazel run //backend:record -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 4k --exposure-us 25.4 --frame-rate-hz 30 --save-recordings-dir ~/Downloads/camera_out --max-frames 100
```

```
bazel run //backend:record -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 100 --frame-rate-hz 5 --output-dir ~/Downloads/camera_out --max-frames 10
```

## Stream from Cameras
```
bazel run //backend:stream -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 10000 --frame-rate-hz 10
```