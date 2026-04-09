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

Note that we had to configure the switch by connecting to <what IP?> and setting the Maximum Transmission Unit (MTU) to ...? TODO

## Discover Cameras
This program is used to discover cameras on the local area network (LAN). This is necessary to get camera related information necessary to record or stream using Aravis.  

To run this program, run the following command:  
`bazel run //backend:discover`  

An example output looks like the following:
```
TODO
```

Note that it takes up to a few minutes for the laptop, once plugged in, to be able to discover the cameras.

## Stream from Cameras
This program will stream footage from the camera and intrinsics provided to a window on the user's screen, allowing them to adjust intrinsics and restart the stream. This is necessary for quick tuning before recording.  

To run this program, run the following command:  
```
bazel run //backend:stream -- --camera <camera> --resolution <resolution> --exposure-us <exposure> --frame-rate-hz <frame rate>
```

Example:  
```
bazel run //backend:stream -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 10000 --frame-rate-hz 10
```

## Record from One Camera
This program just records footage from one camera and writes to disk.

To run this program, run the following command:  
TODO: format better so user knows they can also use max_duration
```
bazel run //backend:record-from-one -- --camera <camera> --resolution <resolution> --exposure-us <exposure> --frame-rate-hz <frame rate> --output-dir <output_dir> --max-frames <max_frames>
```

Example:  
```
bazel run //backend:record-from-one -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 100 --frame-rate-hz 5 --output-dir ~/Downloads/camera_out --max-frames 10
```

## Record from Both Cameras
To run this program, run the following command:  
```
bazel run //backend:record -- --resolution <resolution> --exposure-us <exposure> --frame-rate-hz <frame rate> --output-dir <output_dir> --max-duration <max_duration>
```

Example:
```
bazel run //backend:record -- --resolution 4k --exposure-us 10000 --frame-rate-hz 30.0 --output-dir ~/Downloads/camera_out --max-duration 60
```
