#include <std/mem.pat>
#include <std/io.pat>

struct Frame {
    u16 top;
    u16 left;
    u16 bottom;
    u16 right;
    u32 start;
    u32 end;
    char data[((right - left) * (bottom - top))]@(start + 12);
};

struct Epf {
    u16 frame_count;
    u16 width;
    u16 height;
    u16 unknown;
    u32 toc_offset;
    Frame frames[frame_count]@(toc_offset + 12);
};

Epf epf @0x0;