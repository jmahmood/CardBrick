// Minimal display test to check what's available on the device

use std::process::Command;

fn main() {
    println!("=== CardBrick Display Diagnostics ===");

    // Check if we're running on the device
    println!("1. Device Detection:");
    if std::path::Path::new("/mnt/SDCARD/Tools").exists() {
        println!("   ✓ TrimUI Brick detected");
    } else if std::path::Path::new("/storage/.config/distribution").exists() {
        println!("   ✓ RG35XX Plus detected");
    } else {
        println!("   ✓ Desktop/Unknown device");
    }

    // Check display environment
    println!("\n2. Display Environment:");
    match std::env::var("DISPLAY") {
        Ok(display) => println!("   ✓ DISPLAY: {}", display),
        Err(_) => println!("   ✗ DISPLAY not set"),
    }

    match std::env::var("WAYLAND_DISPLAY") {
        Ok(display) => println!("   ✓ WAYLAND_DISPLAY: {}", display),
        Err(_) => println!("   ✗ WAYLAND_DISPLAY not set"),
    }

    // Check for framebuffer
    println!("\n3. Framebuffer:");
    if std::path::Path::new("/dev/fb0").exists() {
        println!("   ✓ /dev/fb0 exists");
    } else {
        println!("   ✗ /dev/fb0 not found");
    }

    // Check OpenGL
    println!("\n4. OpenGL/Graphics:");
    if let Ok(output) = Command::new("glxinfo").arg("-B").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("OpenGL") {
            println!("   ✓ OpenGL available");
            for line in stdout.lines() {
                if line.contains("OpenGL renderer") || line.contains("OpenGL version") {
                    println!("     {}", line.trim());
                }
            }
        } else {
            println!("   ✗ OpenGL not available");
        }
    } else {
        println!("   ✗ glxinfo not available");
    }

    // Check EGL
    println!("\n5. EGL:");
    if std::path::Path::new("/usr/lib/libEGL.so").exists()
        || std::path::Path::new("/usr/lib/aarch64-linux-gnu/libEGL.so").exists()
        || std::path::Path::new("/lib/libEGL.so").exists()
    {
        println!("   ✓ EGL library found");
    } else {
        println!("   ✗ EGL library not found");
    }

    // Check running processes
    println!("\n6. Graphics Processes:");
    if let Ok(output) = Command::new("ps").arg("aux").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("Xorg") || line.contains("X11") || line.contains("wayland") {
                println!("   ✓ {}", line.trim());
            }
        }
    }

    println!("\n=== End Diagnostics ===");
}
