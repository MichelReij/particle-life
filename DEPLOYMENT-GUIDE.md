# Particle Life Simulation - Deployment Guide

## 🚀 Production Deployment Checklist

### Prerequisites
- ✅ Web server with HTTPS support (required for WebGPU)
- ✅ FTP access to your server
- ✅ Modern browser support (Chrome/Firefox/Safari latest versions)

### Files to Upload via FTP

#### Root Directory Files:
```
index.html          (8.15 KB) - Main application page
main.js             (476 KB)  - Application JavaScript bundle
styles.css          (4.34 KB) - CSS styles
styles.js           (2.11 KB) - Style utilities
joy.js              (16.2 KB) - Joystick controls
*.module.wasm       (2.8 MB)  - WebAssembly binary
.htaccess           (NEW)     - Server configuration
```

#### Folders to Upload:
```
shaders/            (92.8 KB total) - All .wgsl GPU shader files
pkg/                (12 MB total)   - WebAssembly package files
```

### Server Configuration

1. **Upload .htaccess file** (created above) to enable:
   - WebAssembly MIME type support
   - HTTPS redirect (required for WebGPU)
   - Compression and caching
   - Security headers

2. **Verify HTTPS is working** - WebGPU will NOT work over HTTP in production

### Testing After Upload

1. **Open your website URL in a modern browser**
2. **Check browser console** for any loading errors
3. **Test WebGPU support** - particles should render immediately
4. **Test all controls:**
   - Pressure slider (particle count changes)
   - UV Light slider (particle behavior changes)
   - Electrical Activity slider (lightning appears)
   - Zoom controls (mouse wheel/joystick)
   - Pan controls (click-drag/joystick)

### Expected Performance
- **60 FPS** with 3200+ particles
- **Smooth transitions** when adjusting pressure
- **Realistic lightning** with electrical activity
- **No hiccups** during particle count changes

### Troubleshooting

**If particles don't appear:**
- Check browser console for WebGPU support
- Verify HTTPS is enabled
- Check .wasm MIME type is configured

**If performance is poor:**
- Verify files are properly compressed
- Check for JavaScript errors in console
- Test on different browsers

**If lightning doesn't work:**
- Increase electrical activity slider
- Check console for GPU compute errors

### Browser Compatibility
- ✅ Chrome 113+ (full WebGPU support)
- ✅ Firefox 110+ (with WebGPU enabled)
- ✅ Safari 16.4+ (experimental WebGPU)
- ❌ Internet Explorer (not supported)

## 🎯 Your simulation features:
- GPU-accelerated particle physics (up to 6400 particles)
- Real-time lightning generation with collision detection
- Smooth pressure-based particle transitions
- Electromagnetic particle interactions
- Interactive zoom and pan controls
- Temperature-based background color mapping
- Multi-pass post-processing effects

**This is a cutting-edge WebGPU application showcasing advanced GPU compute shaders!** 🚀
