/// Windows screen capture using DXGI Desktop Duplication API

use crate::platform::traits::{ScreenCapture, ScreenRecorder};
use anyhow::Result;
use std::process::{Child, Command, Stdio};
use std::slice;
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Graphics::Direct3D::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;

/// Windows screen capture using DXGI Desktop Duplication
pub struct WindowsScreenCapture {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
}

impl WindowsScreenCapture {
    pub fn new() -> Result<Self> {
        unsafe {
            // Create D3D11 device
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;

            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                None,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[
                    D3D_FEATURE_LEVEL_11_1,
                    D3D_FEATURE_LEVEL_11_0,
                ]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .map_err(|e| anyhow::anyhow!("Failed to create D3D11 device: {:?}", e))?;

            let device = device.ok_or_else(|| anyhow::anyhow!("D3D11 device is null"))?;
            let context = context.ok_or_else(|| anyhow::anyhow!("D3D11 context is null"))?;

            // Get DXGI device and adapter
            let dxgi_device: IDXGIDevice = device.cast()
                .map_err(|e| anyhow::anyhow!("Failed to cast to DXGI device: {:?}", e))?;

            let adapter = dxgi_device.GetAdapter()
                .map_err(|e| anyhow::anyhow!("Failed to get adapter: {:?}", e))?;

            // Get output (display)
            let output = adapter.EnumOutputs(0)
                .map_err(|e| anyhow::anyhow!("Failed to enumerate outputs: {:?}", e))?;

            let output1: IDXGIOutput1 = output.cast()
                .map_err(|e| anyhow::anyhow!("Failed to cast to IDXGIOutput1: {:?}", e))?;

            // Create desktop duplication
            let duplication = output1.DuplicateOutput(&device)
                .map_err(|e| anyhow::anyhow!("Failed to create desktop duplication: {:?}", e))?;

            Ok(Self {
                device,
                context,
                duplication,
            })
        }
    }
}

impl ScreenCapture for WindowsScreenCapture {
    fn capture_screen(&self) -> Result<Vec<u8>> {
        unsafe {
            // Acquire next frame
            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
            let mut desktop_resource: Option<IDXGIResource> = None;

            self.duplication
                .AcquireNextFrame(1000, &mut frame_info, &mut desktop_resource)
                .map_err(|e| anyhow::anyhow!("Failed to acquire frame: {:?}", e))?;

            let desktop_resource = desktop_resource.ok_or_else(|| anyhow::anyhow!("Desktop resource is null"))?;

            // Get texture from resource
            let texture: ID3D11Texture2D = desktop_resource.cast()
                .map_err(|e| anyhow::anyhow!("Failed to cast to texture: {:?}", e))?;

            // Get texture description
            let desc = {
                let mut d = std::mem::zeroed::<D3D11_TEXTURE2D_DESC>();
                texture.GetDesc(&mut d);
                d
            };

            // Create staging texture (CPU-accessible)
            let staging_desc = D3D11_TEXTURE2D_DESC {
                Width: desc.Width,
                Height: desc.Height,
                MipLevels: 1,
                ArraySize: 1,
                Format: desc.Format,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
            };

            let mut staging_texture: Option<ID3D11Texture2D> = None;
            self.device.CreateTexture2D(&staging_desc, None, Some(&mut staging_texture))
                .map_err(|e| anyhow::anyhow!("Failed to create staging texture: {:?}", e))?;

            let staging_texture = staging_texture.ok_or_else(|| anyhow::anyhow!("Staging texture is null"))?;

            // Copy texture to staging
            self.context.CopyResource(&staging_texture, &texture);

            // Map staging texture
            let mut mapped = std::mem::zeroed::<D3D11_MAPPED_SUBRESOURCE>();
            self.context.Map(&staging_texture, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| anyhow::anyhow!("Failed to map texture: {:?}", e))?;

            // Read pixels
            let width = desc.Width as usize;
            let height = desc.Height as usize;
            let pixel_count = width * height * 4;

            let pixels = slice::from_raw_parts(mapped.pData as *const u8, pixel_count);

            // Convert BGRA to RGB
            let mut rgb_pixels = Vec::with_capacity(width * height * 3);
            for chunk in pixels.chunks(4) {
                if chunk.len() >= 4 {
                    rgb_pixels.push(chunk[2]); // R
                    rgb_pixels.push(chunk[1]); // G
                    rgb_pixels.push(chunk[0]); // B
                }
            }

            // Encode as JPEG
            let img = image::RgbImage::from_raw(desc.Width, desc.Height, rgb_pixels)
                .ok_or_else(|| anyhow::anyhow!("Failed to create image from raw pixels"))?;

            let mut jpeg_bytes = Vec::new();
            img.write_to(
                &mut std::io::Cursor::new(&mut jpeg_bytes),
                image::ImageOutputFormat::Jpeg(85),
            )
            .map_err(|e| anyhow::anyhow!("Failed to encode JPEG: {}", e))?;

            // Cleanup
            self.context.Unmap(&staging_texture, 0);
            self.duplication.ReleaseFrame()
                .map_err(|e| anyhow::anyhow!("Failed to release frame: {:?}", e))?;

            Ok(jpeg_bytes)
        }
    }

    fn capture_region(&self, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<Vec<u8>> {
        // For simplicity, capture full screen and crop
        // TODO: Implement actual region capture
        self.capture_screen()
    }
}

/// Windows screen recorder using FFmpeg with gdigrab
pub struct WindowsScreenRecorder {
    child: Mutex<Option<Child>>,
}

impl WindowsScreenRecorder {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
        }
    }
}

impl ScreenRecorder for WindowsScreenRecorder {
    fn start_recording(&self, output_path: &str) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();

        if child_guard.is_some() {
            anyhow::bail!("Recording already in progress");
        }

        // Start FFmpeg with gdigrab input (Windows screen capture)
        let child = Command::new("ffmpeg")
            .args(&[
                "-f",
                "gdigrab",
                "-framerate",
                "5", // 5 FPS
                "-i",
                "desktop",
                "-vf",
                "scale=1280:-1", // Downscale to 720p width
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                "-y", // Overwrite output file
                output_path,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start FFmpeg: {}", e))?;

        *child_guard = Some(child);
        Ok(())
    }

    fn stop_recording(&self) -> Result<()> {
        let mut child_guard = self.child.lock().unwrap();

        if let Some(mut child) = child_guard.take() {
            // Send 'q' to stdin to gracefully stop FFmpeg
            if let Some(mut stdin) = child.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(b"q");
            }

            // Wait for process to exit
            let _ = child.wait();
        }

        Ok(())
    }

    fn is_recording(&self) -> bool {
        self.child.lock().unwrap().is_some()
    }
}

/// Stub screen capture (fallback if initialization fails)
pub struct StubScreenCapture;

impl ScreenCapture for StubScreenCapture {
    fn capture_screen(&self) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("DXGI screen capture not available"))
    }

    fn capture_region(&self, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!("DXGI screen capture not available"))
    }
}

impl Default for WindowsScreenRecorder {
    fn default() -> Self {
        Self::new()
    }
}
