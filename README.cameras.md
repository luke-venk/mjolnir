# Cameras

This project uses 2 [LUCID Vision Labs™ Atlas ATP124S](https://www.edmundoptics.com/p/lucid-vision-labst-atlas-atp124s-mc-sony-imx545-123mp-ip67-monochrome-camera/54295/) cameras with the following specifications:

- GigE Vision v2.0: global industrial camera interface standard that enables high-speed data transmission and device control over Gigabit Ethernet networks
- Resolution: 12.3 Megapixels
- Frame Rate: 42.5 frames per second
- Power over Ethernet
- Global shutter: required for our computer vision frames to be accurate
- Precise Time Synchronization (PTP): required for accurate calculations of where the object implemented
- Monochrome

We have 4 binaries related to using the cameras.

1. Discovery: Finding the cameras on the LAN and their IDs.
2. Stream: Live stream the footage from one of the cameras straight to your laptop, so you can tune intrinsics more quickly.
3. Record with one camera: Recording using one of the cameras on your LAN and write to disk.
4. Record with both cameras: Record simultaneously using both of the cameras on your LAN and write to disk.

## (IMPORTANT) Hardware Setup

1. Connect the switch to power (outlet if indoors, Jackery if outdoors). It will flash a green LED.
2. Connect the RJ45 to M12 connector cables to the switch (RJ45) and cameras (M12). They will flash red then eventually green LEDs.
3. Attach the lenses to the cameras
4. Connect the laptop to the switch using the USB-Ethernet Network Adapter
5. Run the following commands to enable receiving jumbo frames from the switch to your PC
   - `ifconfig`: Check which network interface is connected to the adapter, likely `en<number>`
   - `sudo networksetup -setMTU en<number> 9000`: Set the maximum transmission unit (MTU) to support jumbo packets.
   - `ifconfig en<number>`: Check that the MTU indeed is now 9000

Note that we had to configure the switch by connecting to its IP address and setting the MTU to 9000. However, this only has to be configured once per switch, so this does not need to be repeated.

## Discover Cameras

This program is used to discover cameras on the local area network (LAN). This is necessary to get camera related information necessary to record or stream using Aravis.

To run this program, run the following command:  
`bazel run //backend:discover`

Note that it takes up to a few minutes for the laptop, once plugged in, to be able to discover the cameras.

## Stream from Cameras

This program will stream footage from the camera and intrinsics provided to a window on the user's screen, allowing them to adjust intrinsics and restart the stream. This is necessary for quick tuning before recording.

To run this program, run the following command:

```
bazel run //backend:stream -- --camera <camera> --resolution <resolution> --exposure-us <exposure> --frame-rate-hz <frame rate>
```

Example for Camera 224:
```
bazel run //backend:stream -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 10000 --frame-rate-hz 10
```

Example for Camera 242:
```
bazel run //backend:stream -- --camera "Lucid Vision Labs-ATP124S-M-242700635" --resolution 720p --exposure-us 10000 --frame-rate-hz 10
```

## Record from One Camera

This program just records footage from one camera and writes to disk.

To run this program, run the following command:

Example for Camera 224:
```
bazel run //backend:record_from_one -- --camera "Lucid Vision Labs-ATP124S-M-224300917" --resolution 720p --exposure-us 10000 --frame-rate-hz 2 --output-dir ~/Downloads/camera_out/ --max-duration-s 5 --throwaway-duration-s 5
```

Example for Camera 242:
```
bazel run //backend:record_from_one -- --camera "Lucid Vision Labs-ATP124S-M-242700635" --resolution 720p --exposure-us 10000 --frame-rate-hz 2 --output-dir ~/Downloads/camera_out/ --max-duration-s 5 --throwaway-duration-s 5
```

## Record from Both Cameras

To run this program, run the following command:

```
bazel run //backend:record -- --resolution <resolution> --exposure-us <exposure> --frame-rate-hz <frame rate> --output-dir <output_dir> --max-duration-s <max_duration-s> --throwaway-duration-s <throwaway-duration-s> --interface <interface>
```

You can alternatively specify `--max-frames <max_frames>` instead of max duration.

Example:

```
bazel run //backend:record -- --resolution 4k --exposure-us 10000 --frame-rate-hz 30 --output-dir ~/Downloads/camera_out/test --max-duration-s 10 --throwaway-duration-s 3 --interface en8
```

The `interface` bit tells the binary which network interface the cameras are on so that we make our UDP socket broadcasts from the correct interface. You must have already configured an IP address on the specified network address. You can view your network interfaces via `ifconfig` and set a static IP on an interface in your network settings. On Mac, this is at `System Settings > Network > <the interface>`.
