# TDLR

- If you are using SDL2 on Linux, you should probably dynamically link your libraries.
- If you are on a X86_64 machine, like me, you almost certainly should use Docker to handle the build process.
	- Caching parts of the build process will save a lot of time.
	- Using an ARM64 machine, like a Mac M1, significantly (15x) sped up my compilation times (30 minutes to 2 minutes).
	- My machine is littered with ARM64 and other 32 bit code
- When copying binary files onto the device, extract the microsd card, connect it to your computer, and copy it then.  Do not use finder.  
	- Even USB mode works poorly (at one point it was renaming cardbrick to cardbr~4; straight back to dos days!)


## Why the TrimUI Brick

- Linux-based game devices are great because they are
	- Cheap
	- Attractive
	- Easily available
	- Purposeful, and not ad-driven
	- Wifi-enabled - not cellular-enabled - so no unneeded "required internet" integration ruining the device
- Picked the TrimUI Brick for its 1024x768 screen, but in reality, any of these devices could be used.
- Have never had to build an application for a non-phone / non-desktop before
- Picked Rust to gain experience with a real app, I assume anything could be used if I could work with Rust.

# OS

- I'm running the Brick with MinUI.
- The TrimUI Brick's OS is not a standard Linux OS and is stuck at a relatively old GCC version.
	- In practice, this meant I had to compile on an older version of Debian
- MinUI does not have a built-in SSH server (unlike some other devices like the Powkiddy RGB30 or the Ambernic RG35XX).  It does not have rsync, nor does it have ssh.  
	- Added SSH by dropping an application

# AI

- I used both Gemini 2.5 Pro and ChatGPT o3 for help
- Was mislead, repeatedly, about how to deal with setbacks.
	- Gemini was bad for flattery and losing context.
	- o3 was far too confident and at some point I started thinking it was 
- However, both were invaluable for setup and handling compilation issues.
	- Impossible to do so otherwise; no one to consult, or to learn from from people nearby.
	- The last time I did static linking of anything was probably in 2003 when building C++ stuff in University.  Did not even have internet access then and used paper books to find information.


# Actual issues

## Dynamic vs. Static Linking

### Take 1: Dynamic
- Originally was going to try Dynamic linking of libraries
- Was continuously required to add more libraries
- After adding 15-20 libraries, failed with a message about GLIBC.  I did not understand this and thought it was another library issue so I decided maybe stati clinking was the way to go.
	> ./cardbrick: /lib/libc.so.6: version GLIBC_2.34' not found (required by ./cardbrick)


### Take 2: Static
- Switched to MUSL; the GLIB issue scared me, and I was not using Docker for the builds at the time.
- With the change to MUSL, decided to try to statically link because having 10-20 files to distribute each time sounded like a pain
- Had to compile from source (-_-)
	- Apparently SDL moved from autotools to meson (unless you get from github)
		- Sometimes have to use true/false, other times enabled/disabled.   
		- Version that was shipping with the image I was using was too old to compile, but could install with pip
	- AI initially suggested use a cross file, but I didn't need one as I was compiling on a Dockerfile 
	- SDL is not just one library but three in my case; SDL_image and SDL_tff
		- When you fail to include, Cargo get errors like this when compiling:
			ld: cannot find -lSDL2_image
			ld: cannot find -lSDL2_ttf
		- Cargo's "Bundled" feature for SDL2 does not include SDL2_image, nor SDL2_ttf
- pkg-config
	- Often I thought I had the files available but I did not and the AI kept inaccurately suggesting it was because pkg-config was missing.

- Created a binary that started, but lead to a blank screen.  
	- Initially, the problem seemed to be some weirdness when copying files using Mac Finder
		- I've literally never had problems with the file copy being incomplete since the early 2000s, so this was a huge shock
	- SDL2 was linked but it did not having the graphics enabled (??)
	- Could not use strace or other tools (despite copying over toybox), so I was never 100% certain what the cause of this problem was.

- I could not enable graphics on a statically-linked SDL2
	- I was able to create the statically linked SDL2, but it would not include KMSDRM and it would crash in cargo
```
undefined reference to `KMSDRM_bootstrap`
```
	- AI suggested I could use KMSDRM or FBDEV
	- FBDEV
		- Probably the easiest method
		- Not supported for more than 10 years now.
		- Removed from all official SDL2 libraries
			- Some unofficial ports exist
			- Had already built with SDL2 so could not use this.

	- To add KMSDRM, I needed three libraries.  I could create the .a file for libdrm and libgbm, but not for libegl.

```
#18 9.828 -- Checking for modules 'libdrm;gbm;egl'
#18 9.831 --   No package 'gbm' found
#18 9.833 --   No package 'egl' found
...
#18 10.59 --   SDL_KMSDRM                  (Wanted: ON): OFF
````
	- Dynamically linking something after statically linking seemed wrong.
	- Most examples online involved C, not Rust/Cargo.  As such, it wasn't easy to figure out how to adapt them for my own use case.
	- "In my experience, static linking works reliably on Windows but poorly on Linux." - https://nullprogram.com/blog/2023/01/08/
	- "However, we encourage you to not statically link for various technical and moral reasons" - https://wiki.libsdl.org/SDL2/Installation

### Take 3: Dynamic

- After a lot of playing, I realized I did not want to learn more about libdrm/libgbm/libegl. I also didn't want to rewrite my code to use something other than SDL2
- After going back over my previous steps, I did realize that the issue I had with GLIBC was from when I compiling on my desktop and before I had switched to using a Docker-based build system.
- Debian Buster images were invaluable for this purpose.  
	- had to do some less savoury things to get enviroment built since it is not supported anymore.
		echo "deb [trusted=yes] http://archive.debian.org/debian bullseye main" > /etc/apt/sources.list
	- Version of glibc was compatible with my device
- Did not need any custom compilation or anything; still fundamentally a one-click install

# Thoughts

- Rust was a joy to work with, even when the AI did not work very well with it.
- I would have liked it if I was able to use MUSL to link my application, to avoid having to use an ancient version of Debian. However, in that case I would almost certainly have to distribute the libraries too, becuase they are linked to glibc, and some of the libraries require dynamic linking.  
	- I wonder if this meant I would inevitably run into a GCC issue
- Maybe using SDL2 at the start wasn't the smartest idea.  Works fine on my PC though.
- When looking through the MESA documentation, I feel like I misunderstood the relationship between GBM and EGL.
