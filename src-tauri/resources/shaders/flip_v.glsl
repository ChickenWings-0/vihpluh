//!HOOK MAIN
//!BIND HOOKED
//!DESC vihpluh: flip vertically

// Flips the picture on the GPU. Unlike a video filter this keeps zero-copy hardware decoding
// active, and subtitles (drawn after this stage) stay readable.
vec4 hook() {
    return HOOKED_tex(vec2(HOOKED_pos.x, 1.0 - HOOKED_pos.y));
}
