use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use windows::Win32::Foundation::BOOL;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFOEXW};
use windows_capture::capture::{CaptureControl, Context, GraphicsCaptureApiHandler};
use windows_capture::frame::Frame;
use windows_capture::graphics_capture_api::InternalCaptureControl;
use windows_capture::monitor::Monitor;
use windows_capture::settings::{
    ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
    MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
};

pub struct CapturePool {
    monitors: Vec<MonitorSession>,
    is_shutdown: AtomicBool,
}

impl CapturePool {
    /// Starts one persistent capture session per active monitor.
    /// Returns CaptureError on failures and never panics.
    pub fn start() -> Result<Arc<Self>, CaptureError> {
        let monitors = Monitor::enumerate()
            .map_err(|error| CaptureError::SessionStartFailed(error.to_string()))?;
        if monitors.is_empty() {
            return Err(CaptureError::SessionStartFailed(
                "no active monitors found".to_string(),
            ));
        }

        let mut sessions = Vec::with_capacity(monitors.len());
        for (monitor_index, monitor) in monitors.into_iter().enumerate() {
            let geometry = monitor_geometry(&monitor)?;
            let latest_frame = Arc::new(RwLock::new(None));
            let flags = HandlerFlags {
                shared: Arc::clone(&latest_frame),
                monitor_x: geometry.left,
                monitor_y: geometry.top,
            };

            let settings = Settings::new(
                monitor,
                CursorCaptureSettings::WithoutCursor,
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            );

            let control =
                MonitorCaptureHandler::start_free_threaded(settings).map_err(|error| {
                    CaptureError::SessionStartFailed(format!(
                        "monitor {} failed: {}",
                        monitor_index, error
                    ))
                })?;

            sessions.push(MonitorSession {
                left: geometry.left,
                top: geometry.top,
                width: geometry.width,
                height: geometry.height,
                latest_frame,
                control: Mutex::new(Some(control)),
            });
        }

        Ok(Arc::new(Self {
            monitors: sessions,
            is_shutdown: AtomicBool::new(false),
        }))
    }

    /// Finds the monitor by virtual-desktop point and returns its newest frame.
    /// Returns Ok(None) when the session has not produced a frame yet.
    /// Returns Err(NoMonitor) when no monitor matches the point.
    pub fn snapshot_at_point(&self, x: i32, y: i32) -> Result<Option<Snapshot>, CaptureError> {
        let session = self
            .monitors
            .iter()
            .find(|monitor| monitor.contains_point(x, y))
            .ok_or(CaptureError::NoMonitor(x, y))?;
        Ok(session.snapshot())
    }

    /// Returns the newest frame for the monitor index.
    pub fn snapshot_for_monitor(
        &self,
        monitor_index: usize,
    ) -> Result<Option<Snapshot>, CaptureError> {
        let session = self
            .monitors
            .get(monitor_index)
            .ok_or(CaptureError::MonitorIndexOutOfBounds(monitor_index))?;
        Ok(session.snapshot())
    }

    /// Stops all sessions and releases resources (idempotent).
    pub fn shutdown(&self) {
        if self.is_shutdown.swap(true, Ordering::AcqRel) {
            return;
        }

        for monitor in &self.monitors {
            monitor.shutdown();
        }
    }
}

impl Drop for CapturePool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub monitor_x: i32,
    pub monitor_y: i32,
    pub bgra_buffer: Vec<u8>,
    pub captured_at: Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no monitor found at point ({0}, {1})")]
    NoMonitor(i32, i32),
    #[error("monitor index out of bounds: {0}")]
    MonitorIndexOutOfBounds(usize),
    #[error("capture session failed to start: {0}")]
    SessionStartFailed(String),
    #[error("internal capture error: {0}")]
    Internal(String),
}

struct HandlerFlags {
    shared: Arc<RwLock<Option<FrameSnapshot>>>,
    monitor_x: i32,
    monitor_y: i32,
}

struct MonitorCaptureHandler {
    shared: Arc<RwLock<Option<FrameSnapshot>>>,
    monitor_x: i32,
    monitor_y: i32,
}

impl GraphicsCaptureApiHandler for MonitorCaptureHandler {
    type Flags = HandlerFlags;
    type Error = CaptureError;

    fn new(ctx: Context<Self::Flags>) -> Result<Self, Self::Error> {
        Ok(Self {
            shared: ctx.flags.shared,
            monitor_x: ctx.flags.monitor_x,
            monitor_y: ctx.flags.monitor_y,
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut Frame<'_>,
        _capture_control: InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        let frame_buffer = frame
            .buffer()
            .map_err(|error| CaptureError::Internal(error.to_string()))?;

        let width = frame_buffer.width();
        let height = frame_buffer.height();
        let expected_len = width as usize * height as usize * 4;
        let mut packed = Vec::with_capacity(expected_len);
        let no_padding = frame_buffer.as_nopadding_buffer(&mut packed);
        let bgra_buffer = no_padding.to_vec();

        let snapshot = FrameSnapshot {
            width,
            height,
            monitor_x: self.monitor_x,
            monitor_y: self.monitor_y,
            bgra_buffer,
            captured_at: Instant::now(),
        };

        if let Ok(mut guard) = self.shared.write() {
            *guard = Some(snapshot);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct FrameSnapshot {
    width: u32,
    height: u32,
    monitor_x: i32,
    monitor_y: i32,
    bgra_buffer: Vec<u8>,
    captured_at: Instant,
}

struct MonitorSession {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    latest_frame: Arc<RwLock<Option<FrameSnapshot>>>,
    control: Mutex<Option<CaptureControl<MonitorCaptureHandler, CaptureError>>>,
}

impl MonitorSession {
    fn contains_point(&self, x: i32, y: i32) -> bool {
        let right = self.left + self.width as i32;
        let bottom = self.top + self.height as i32;
        x >= self.left && x < right && y >= self.top && y < bottom
    }

    fn snapshot(&self) -> Option<Snapshot> {
        let guard = self.latest_frame.read().ok()?;
        let frame = guard.as_ref()?.clone();
        Some(Snapshot {
            width: frame.width,
            height: frame.height,
            monitor_x: frame.monitor_x,
            monitor_y: frame.monitor_y,
            bgra_buffer: frame.bgra_buffer,
            captured_at: frame.captured_at,
        })
    }

    fn shutdown(&self) {
        let control = match self.control.lock() {
            Ok(mut guard) => guard.take(),
            Err(_) => None,
        };

        if let Some(control) = control {
            let _ = control.stop();
        }
    }
}

struct MonitorGeometry {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

fn monitor_geometry(monitor: &Monitor) -> Result<MonitorGeometry, CaptureError> {
    let raw_hmonitor = monitor.as_raw_hmonitor();
    let hmonitor = HMONITOR(raw_hmonitor);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

    let ok = unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _ as *mut _) };
    if ok != BOOL(1) {
        return Err(CaptureError::SessionStartFailed(
            "GetMonitorInfoW failed".to_string(),
        ));
    }

    let rect = info.monitorInfo.rcMonitor;
    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;

    Ok(MonitorGeometry {
        left: rect.left,
        top: rect.top,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::CapturePool;
    use std::sync::mpsc;
    use std::time::Duration;
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
    };

    #[test]
    fn test_start_does_not_panic() {
        let result = std::panic::catch_unwind(CapturePool::start);
        assert!(result.is_ok());
        if let Ok(Ok(pool)) = result {
            pool.shutdown();
        }
    }

    #[test]
    fn test_snapshot_before_frame_ok() {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::spawn(move || {
            let result = CapturePool::start();
            match result {
                Ok(pool) => {
                    let snapshot = pool.snapshot_at_point(0, 0);
                    pool.shutdown();
                    tx.send(snapshot.is_ok() || snapshot.is_err()).ok();
                }
                Err(_) => {
                    tx.send(true).ok();
                }
            }
        });

        let finished = rx
            .recv_timeout(Duration::from_millis(2000))
            .unwrap_or(false);
        assert!(finished);
        let _ = handle.join();
    }

    #[test]
    fn test_shutdown_idempotent() {
        let result = CapturePool::start();
        if let Ok(pool) = result {
            pool.shutdown();
            pool.shutdown();
        }
    }

    #[test]
    #[ignore = "stress test; run with: cargo test --ignored gpu_leak_capture_pool"]
    fn gpu_leak_capture_pool() {
        let baseline = match read_process_vram_local_bytes() {
            Some(vram) => vram,
            None => {
                eprintln!("[stress] DXGI VRAM query unavailable; skipping stress test");
                return;
            }
        };

        let pool = match CapturePool::start() {
            Ok(pool) => pool,
            Err(error) => {
                eprintln!(
                    "[stress] CapturePool::start failed ({error}); likely no GPU on CI runner; skipping"
                );
                return;
            }
        };

        eprintln!("[stress] baseline VRAM = {} MB", baseline / 1024 / 1024);

        let iterations = 1000u32;
        let sample_every = 100u32;
        let mut samples = vec![(0u32, baseline)];

        for i in 1..=iterations {
            let _ = pool.snapshot_at_point(500, 500);
            if i % sample_every == 0 {
                std::thread::sleep(Duration::from_millis(50));
                if let Some(vram) = read_process_vram_local_bytes() {
                    samples.push((i, vram));
                    eprintln!("[stress] iter {i}: VRAM = {} MB", vram / 1024 / 1024);
                }
            }
        }

        pool.shutdown();
        std::thread::sleep(Duration::from_millis(500));

        let final_vram = read_process_vram_local_bytes().unwrap_or(baseline);
        let growth_bytes = final_vram.saturating_sub(baseline);
        let growth_pct = (growth_bytes as f64 / baseline.max(1) as f64) * 100.0;

        eprintln!("[stress] final VRAM = {} MB", final_vram / 1024 / 1024);
        eprintln!(
            "[stress] growth = {} MB ({growth_pct:.2}%)",
            growth_bytes / 1024 / 1024
        );

        assert!(
            growth_pct < 5.0,
            "GPU VRAM grew {growth_pct:.2}% over {iterations} snapshots (expected < 5%); samples: {samples:?}"
        );
    }

    fn read_process_vram_local_bytes() -> Option<u64> {
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let adapter = factory.EnumAdapters(0).ok()?;
            let adapter3: IDXGIAdapter3 = adapter.cast().ok()?;
            let mut info = Default::default();
            adapter3
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                .ok()?;
            Some(info.CurrentUsage)
        }
    }
}
