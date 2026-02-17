# Developer README

## Usage
Run everything with:  `make`
Bring everything down with:  `make down`

Check Makefile to see what these do, but basically just using Docker Compose.

## Getting Dummy Data from Backend
Test server is alive:  `curl localhost:8000/api/hello_world`

Get dummy data:  `curl localhost:8000/api/dummy`
* Saves data to data/
* Both results.json (which provides the URLs to the images), as well as the images

Get images from a given throw:  `curl localhost:8000/media/<UUID>/image<1|2|3>.jpg -o <output>.jpg`