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
        println!("🎵 Initializing audio system...");
        
        // Create audio output stream
        let (_stream, stream_handle) = OutputStream::try_default()?;
        println!("✅ Audio output stream created");
        
        // Create sink for background music
        let background_sink = Arc::new(Sink::try_new(&stream_handle)?);
        
        // Start background music
        Self::start_background_music(&background_sink)?;
        
        println!("🎵 Background music started");
        
        Ok(AudioManager {
            _stream,
            background_sink,
            current_volume: 70, // Default to 70%
        })
    }
    
    /// Start playing the background brainwaves music on loop
    fn start_background_music(sink: &Arc<Sink>) -> Result<(), Box<dyn std::error::Error>> {
        let audio_path = Path::new("assets/audio/brainwaves.mp3");
        
        if !audio_path.exists() {
            return Err(format!("Audio file not found: {}", audio_path.display()).into());
        }
        
        println!("🎵 Loading background music: {}", audio_path.display());
        
        // Load and play the audio file
        let file = File::open(audio_path)?;
        let source = Decoder::new(BufReader::new(file))?;
        
        // Create a repeating source that loops forever
        let looping_source = source.repeat_infinite();
        
        // Set volume to default level (70%)
        let volume_adjusted = looping_source.amplify(0.7);
        
        sink.append(volume_adjusted);
        sink.play();
        
        println!("✅ Background music loaded and playing on loop at 70% volume");
        Ok(())
    }
    
    /// Set background music volume (0-100)
    pub fn set_volume(&mut self, volume: u8) {
        let old_volume = self.current_volume;
        self.current_volume = volume.clamp(0, 100);
        
        if self.current_volume == 0 {
            // Volume 0: pause the music
            self.background_sink.pause();
            println!("🔇 Volume set to 0% - music paused");
        } else {
            // Convert 0-100 to 0.0-1.0 range
            let volume_float = self.current_volume as f32 / 100.0;
            self.background_sink.set_volume(volume_float);
            
            // If we were at volume 0 before, resume playback
            if old_volume == 0 && self.current_volume > 0 {
                self.background_sink.play();
                println!("▶️ Music resumed at {}% volume", self.current_volume);
            } else {
                println!("🔊 Volume set to {}%", self.current_volume);
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