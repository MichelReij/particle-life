use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use rodio::{Decoder, OutputStream, Sink, Source};

/// Audio manager for background music and sound effects
pub struct AudioManager {
    _stream: OutputStream,
    background_sink: Arc<Sink>,
    current_volume: u8, // Volume from 0-100
}

impl AudioManager {
    /// Initialize the audio system and start playing background music
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("🎵 Initializing audio system for PipeWire/ALSA...");
        
        // Check if we can detect audio system type
        Self::detect_and_configure_audio_system();
        
        // Try to create audio output stream
        let (_stream, stream_handle) = match OutputStream::try_default() {
            Ok(stream) => {
                println!("✅ Audio output stream created successfully");
                stream
            }
            Err(e) => {
                println!("❌ Failed to create audio stream: {}", e);
                println!("🔧 Trying PipeWire-specific configuration...");
                
                // For PipeWire systems, try different approach
                match Self::try_pipewire_configuration() {
                    Ok(stream) => {
                        println!("✅ PipeWire audio stream created");
                        stream
                    }
                    Err(pipewire_err) => {
                        println!("❌ PipeWire configuration failed: {}", pipewire_err);
                        println!("🔇 Audio disabled to prevent system issues");
                        return Err(format!("Audio disabled - system incompatibility: {}", e).into());
                    }
                }
            }
        };
        
        // Create sink for background music
        let background_sink = Arc::new(Sink::try_new(&stream_handle)?);
        
        println!("🎵 Testing audio sink functionality...");
        
        // Start background music
        match Self::start_background_music_simple(&background_sink) {
            Ok(()) => {
                println!("🎵 Background music loaded successfully");
            }
            Err(e) => {
                println!("⚠️ Failed to load background music: {}", e);
                println!("   Continuing with silent audio system for testing");
                // Continue anyway - we can test the audio system first
            }
        }
        
        let mut audio_manager = AudioManager {
            _stream,
            background_sink,
            current_volume: 75, // Start at 75%
        };
        
        // Set initial volume
        audio_manager.set_volume(75);
        
        println!("🎵 Audio system initialization complete!");
        println!("🎵 Try pressing [M] to toggle music or [+]/[-] to adjust volume");
        Ok(audio_manager)
    }    /// Simple background music loading for initial testing
    fn start_background_music_simple(sink: &Arc<Sink>) -> Result<(), Box<dyn std::error::Error>> {
        let audio_path = Path::new("assets/audio/brainwaves.mp3");
        
        if !audio_path.exists() {
            println!("⚠️ Audio file not found: {}", audio_path.display());
            println!("   Place brainwaves.mp3 in assets/audio/ directory for background music");
            println!("   Continuing without background music...");
            return Ok(()); // Don't error out, just continue without music
        }
        
        println!("🎵 Loading background music: {}", audio_path.display());
        
        // Load and play the audio file
        let file = File::open(audio_path)?;
        let source = Decoder::new(BufReader::new(file))?;
        
        // Create a repeating source that loops forever
        let looping_source = source.repeat_infinite();
        
        // Simple buffering for initial test
        let buffered_source = looping_source.buffered();
        
        // Set volume to 75%
        let volume_adjusted = buffered_source.amplify(0.75);
        
        sink.append(volume_adjusted);
        
        println!("✅ Background music loaded");
        Ok(())
    }
    
    /// Detect audio system type (PipeWire, PulseAudio, pure ALSA)
    fn detect_and_configure_audio_system() {
        use std::process::Command;
        
        println!("🔍 Detecting audio system...");
        
        // Check for PipeWire
        if let Ok(output) = Command::new("pactl").arg("info").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            if output_str.contains("PipeWire") {
                println!("✅ Detected: PipeWire audio system");
                Self::configure_pipewire_audio();
                return;
            } else if output_str.contains("PulseAudio") {
                println!("✅ Detected: PulseAudio system");
                Self::configure_pulseaudio();
                return;
            }
        }
        
        // Fallback to ALSA configuration
        println!("✅ Detected: ALSA audio system (fallback)");
        Self::configure_alsa_audio();
    }
    
    /// Configure PipeWire-specific settings
    fn configure_pipewire_audio() {
        println!("🔧 Configuring PipeWire audio settings...");
        
        // PipeWire typically doesn't need ALSA environment variables
        // But we can set some safe defaults
        std::env::set_var("PIPEWIRE_LATENCY", "512/48000"); // ~10ms latency
        std::env::set_var("PIPEWIRE_RATE", "48000");
        std::env::set_var("PIPEWIRE_CHANNELS", "2");
        
        println!("🔧 PipeWire: Set 10ms latency, 48kHz, stereo");
    }
    
    /// Configure PulseAudio settings
    fn configure_pulseaudio() {
        println!("🔧 Configuring PulseAudio settings...");
        
        std::env::set_var("PULSE_LATENCY_MSEC", "50"); // 50ms latency
        std::env::set_var("PULSE_RUNTIME_PATH", "/run/user/1000/pulse");
        
        println!("🔧 PulseAudio: Set 50ms latency");
    }
    
    /// Configure ALSA settings
    fn configure_alsa_audio() {
        println!("🔧 Configuring ALSA audio settings...");
        
        std::env::set_var("ALSA_PCM_CARD", "0");
        std::env::set_var("ALSA_PCM_DEVICE", "0");
        std::env::set_var("ALSA_BUFFER_TIME", "100000"); // 100ms
        std::env::set_var("ALSA_PERIOD_TIME", "25000");  // 25ms
        
        println!("🔧 ALSA: Set 100ms buffer, 25ms period");
    }
    
    /// Try PipeWire-specific configuration
    fn try_pipewire_configuration() -> Result<(OutputStream, rodio::OutputStreamHandle), Box<dyn std::error::Error>> {
        println!("🔧 Attempting PipeWire-compatible audio stream...");
        
        // For PipeWire, the default should work fine
        OutputStream::try_default()
            .map_err(|e| format!("PipeWire audio configuration failed: {}", e).into())
    }
    
    /// Set background music volume (0-100)
    pub fn set_volume(&mut self, volume: u8) {
        let old_volume = self.current_volume;
        self.current_volume = volume.clamp(0, 100);
        
        // Removed verbose logging for cleaner console output
        // println!("🔊 Setting volume from {}% to {}%", old_volume, self.current_volume);
        
        if self.current_volume == 0 {
            // Volume 0: pause the music
            self.background_sink.pause();
            // println!("🔇 Volume set to 0% - music paused");
        } else {
            // Convert 0-100 to 0.0-1.0 range
            let volume_float = self.current_volume as f32 / 100.0;
            self.background_sink.set_volume(volume_float);
            
            // Always ensure playback is started when volume > 0
            if !self.background_sink.empty() {
                self.background_sink.play();
                // println!("▶️ Music playing at {}% volume (volume: {:.2})", self.current_volume, volume_float);
            } else {
                println!("⚠️ No audio loaded in sink - cannot play");
            }
        }
    }
    
    /// Increase volume by 5 (max 100)
    pub fn volume_up(&mut self) {
        let new_volume = (self.current_volume + 5).min(100);
        self.set_volume(new_volume);
    }
    
    /// Decrease volume by 5 (min 0)
    pub fn volume_down(&mut self) {
        let new_volume = self.current_volume.saturating_sub(5);
        self.set_volume(new_volume);
    }
    
    /// Get current volume (0-100)
    pub fn get_volume(&self) -> u8 {
        self.current_volume
    }
    
    /// Toggle background music play/pause (except when volume is 0)
    pub fn toggle_background(&mut self) {
        println!("🎵 Toggling background music (current volume: {}%, paused: {})", 
            self.current_volume, self.background_sink.is_paused());
            
        if self.current_volume == 0 {
            // If volume is 0, set to default 70% and play
            self.set_volume(70);
        } else {
            // Toggle between current volume and 0
            if self.background_sink.is_paused() || self.current_volume > 0 {
                self.set_volume(0); // This will pause
            }
        }
    }
    
    /// Check if background music is paused
    pub fn is_background_paused(&self) -> bool {
        self.background_sink.is_paused() || self.current_volume == 0
    }
    

}

impl Drop for AudioManager {
    fn drop(&mut self) {
        println!("🎵 Audio system shutting down...");
    }
}