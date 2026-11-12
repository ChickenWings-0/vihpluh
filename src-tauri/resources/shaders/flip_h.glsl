//!HOOK MAIN
//!BIND HOOKED
//!DESC vihpluh: mirror horizontally

// Mirrors the picture on the GPU. Unlike a video filter this keeps zero-copy hardware decoding
// active, and subtitles (drawn after this stage) stay readable.
vec4 hook() {
    return HOOKED_tex(vec2(1.0 - HOOKED_pos.x, HOOKED_pos.y));
}
