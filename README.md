# Using Computer Vision to Detect Distances and Infractions in Throwing Events
## Project Overview
The goal of this project is to engineer a system to assist officials in detecting infractions and measuring distances in the 4 throwing events, including shot put, discus throw, hammer throw, and javelin throw. The system is designed to identify foot infractions (i.e., stepping out of bounds) as well as determine whether the implement lands outside the legal sector. Additionally, the system reports the distance from the throwing circle to the landing point.

Current officiating in these events relies heavily on human judgment, which can be error-prone and inconsistent. Our objective is to improve the accuracy and efficiency of officiating while minimizing the technical burden placed on referees.

The system integrates ground cameras equipped with computer vision, a ground-based sensing and processing pipeline, and a graphical user interface allowing referees to understand system decisions in an intuitive and accessible manner.

## Team Mjölnir
The team's name "Mjölnir" is named after the legendary hammer of Thor, the Norse god of thunder. Mjölnir infamously returns to Thor when he throws it, and since our system helps officiate hammer throw, we thought the name was fitting. Also, in the Marvel Cinematic Universe, the synthetic android "Vision" is one of the few beings able to lift Mjölnir — computer vision...

Anyways, the following engineers contributed to this project.
- Eloghosa Eguakun
- Yash Jain
- Alex Lozano
- Rushil Randhar
- Gautam Rao
- Arushi Sadam
- Owen Scott
- Luke Venkataramanan
- Max Wiesenfeld

## Repository Structure
This section outlines the purpose of each directory in the repository.

### Camera Capture
The code that handles ingesting frames from the cameras, synchronizing the cameras, and recording the footage will live here. The functionality of this directory is separate from that of computer_vision, but the outputs of the camera_capture system will be fed as inputs to the computer_vision system.

### Computer Vision
The code that is responsible for perception and making decisions regarding measuring distance and sector violations will live here. This directory should include object detection, triangulation of rays, and outputting decisions regarding distances and sector violations.

### Circle Infractions

### Frontend
The graphical user interface for our web application will live here, and the frontend will be written React / Next.js (TypeScript).

### Interfaces
The code defining message formats will live here. Because our system includes several independent machines, it is important to have a standard message format defined between applications so they can talk to each other reliably (note: I copied this last phrase directly from Owen's README.md in the TREL monorepo in message_formats/). 

### Analysis
Any code or diagrams that help us inform our design decisions for this system should go here. This includes any MATLAB or Python visualizations, or any useful diagrams.
