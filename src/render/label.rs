// src/render/label.rs
use bytemuck::{Pod, Zeroable};
use crate::scene::SceneNode;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct LabelVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}

impl LabelVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// Embedded 8x8 bitmap font for ASCII 32-126 (95 characters).
// Each u64 encodes 8 rows top-to-bottom, each byte is one row, MSB = leftmost pixel.
#[rustfmt::skip]
const FONT_BITMAP: [u64; 95] = [
    0x00_00_00_00_00_00_00_00, // 32 ' '
    0x18_18_18_18_18_00_18_00, // 33 '!'
    0x6C_6C_00_00_00_00_00_00, // 34 '"'
    0x6C_FE_6C_6C_FE_6C_00_00, // 35 '#'
    0x18_3E_60_3C_06_7C_18_00, // 36 '$'
    0x62_66_0C_18_30_66_46_00, // 37 '%'
    0x3C_66_3C_38_67_66_3F_00, // 38 '&'
    0x18_18_00_00_00_00_00_00, // 39 '''
    0x0C_18_30_30_30_18_0C_00, // 40 '('
    0x30_18_0C_0C_0C_18_30_00, // 41 ')'
    0x00_66_3C_FF_3C_66_00_00, // 42 '*'
    0x00_18_18_7E_18_18_00_00, // 43 '+'
    0x00_00_00_00_00_18_18_30, // 44 ','
    0x00_00_00_7E_00_00_00_00, // 45 '-'
    0x00_00_00_00_00_18_18_00, // 46 '.'
    0x02_06_0C_18_30_60_40_00, // 47 '/'
    0x3C_66_6E_7E_76_66_3C_00, // 48 '0'
    0x18_38_18_18_18_18_7E_00, // 49 '1'
    0x3C_66_06_0C_18_30_7E_00, // 50 '2'
    0x3C_66_06_1C_06_66_3C_00, // 51 '3'
    0x0C_1C_3C_6C_7E_0C_0C_00, // 52 '4'
    0x7E_60_7C_06_06_66_3C_00, // 53 '5'
    0x1C_30_60_7C_66_66_3C_00, // 54 '6'
    0x7E_06_0C_18_30_30_30_00, // 55 '7'
    0x3C_66_66_3C_66_66_3C_00, // 56 '8'
    0x3C_66_66_3E_06_0C_38_00, // 57 '9'
    0x00_18_18_00_18_18_00_00, // 58 ':'
    0x00_18_18_00_18_18_30_00, // 59 ';'
    0x0C_18_30_60_30_18_0C_00, // 60 '<'
    0x00_00_7E_00_7E_00_00_00, // 61 '='
    0x30_18_0C_06_0C_18_30_00, // 62 '>'
    0x3C_66_06_0C_18_00_18_00, // 63 '?'
    0x3C_66_6E_6A_6E_60_3C_00, // 64 '@'
    0x18_3C_66_66_7E_66_66_00, // 65 'A'
    0x7C_66_66_7C_66_66_7C_00, // 66 'B'
    0x3C_66_60_60_60_66_3C_00, // 67 'C'
    0x78_6C_66_66_66_6C_78_00, // 68 'D'
    0x7E_60_60_7C_60_60_7E_00, // 69 'E'
    0x7E_60_60_7C_60_60_60_00, // 70 'F'
    0x3C_66_60_6E_66_66_3E_00, // 71 'G'
    0x66_66_66_7E_66_66_66_00, // 72 'H'
    0x3C_18_18_18_18_18_3C_00, // 73 'I'
    0x1E_0C_0C_0C_0C_6C_38_00, // 74 'J'
    0x66_6C_78_70_78_6C_66_00, // 75 'K'
    0x60_60_60_60_60_60_7E_00, // 76 'L'
    0x63_77_7F_6B_63_63_63_00, // 77 'M'
    0x66_76_7E_7E_6E_66_66_00, // 78 'N'
    0x3C_66_66_66_66_66_3C_00, // 79 'O'
    0x7C_66_66_7C_60_60_60_00, // 80 'P'
    0x3C_66_66_66_6A_6C_36_00, // 81 'Q'
    0x7C_66_66_7C_6C_66_66_00, // 82 'R'
    0x3C_66_60_3C_06_66_3C_00, // 83 'S'
    0x7E_18_18_18_18_18_18_00, // 84 'T'
    0x66_66_66_66_66_66_3C_00, // 85 'U'
    0x66_66_66_66_66_3C_18_00, // 86 'V'
    0x63_63_63_6B_7F_77_63_00, // 87 'W'
    0x66_66_3C_18_3C_66_66_00, // 88 'X'
    0x66_66_66_3C_18_18_18_00, // 89 'Y'
    0x7E_06_0C_18_30_60_7E_00, // 90 'Z'
    0x3C_30_30_30_30_30_3C_00, // 91 '['
    0x40_60_30_18_0C_06_02_00, // 92 '\'
    0x3C_0C_0C_0C_0C_0C_3C_00, // 93 ']'
    0x18_3C_66_00_00_00_00_00, // 94 '^'
    0x00_00_00_00_00_00_FF_00, // 95 '_'
    0x30_18_0C_00_00_00_00_00, // 96 '`'
    0x00_00_3C_06_3E_66_3E_00, // 97 'a'
    0x60_60_7C_66_66_66_7C_00, // 98 'b'
    0x00_00_3C_60_60_60_3C_00, // 99 'c'
    0x06_06_3E_66_66_66_3E_00, // 100 'd'
    0x00_00_3C_66_7E_60_3C_00, // 101 'e'
    0x1C_30_7C_30_30_30_30_00, // 102 'f'
    0x00_00_3E_66_66_3E_06_3C, // 103 'g'
    0x60_60_7C_66_66_66_66_00, // 104 'h'
    0x18_00_38_18_18_18_3C_00, // 105 'i'
    0x0C_00_1C_0C_0C_0C_6C_38, // 106 'j'
    0x60_60_66_6C_78_6C_66_00, // 107 'k'
    0x38_18_18_18_18_18_3C_00, // 108 'l'
    0x00_00_66_7F_7F_6B_63_00, // 109 'm'
    0x00_00_7C_66_66_66_66_00, // 110 'n'
    0x00_00_3C_66_66_66_3C_00, // 111 'o'
    0x00_00_7C_66_66_7C_60_60, // 112 'p'
    0x00_00_3E_66_66_3E_06_06, // 113 'q'
    0x00_00_7C_66_60_60_60_00, // 114 'r'
    0x00_00_3E_60_3C_06_7C_00, // 115 's'
    0x30_30_7C_30_30_30_1C_00, // 116 't'
    0x00_00_66_66_66_66_3E_00, // 117 'u'
    0x00_00_66_66_66_3C_18_00, // 118 'v'
    0x00_00_63_6B_7F_7F_36_00, // 119 'w'
    0x00_00_66_3C_18_3C_66_00, // 120 'x'
    0x00_00_66_66_66_3E_06_3C, // 121 'y'
    0x00_00_7E_0C_18_30_7E_00, // 122 'z'
    0x0C_18_18_30_18_18_0C_00, // 123 '{'
    0x18_18_18_18_18_18_18_00, // 124 '|'
    0x30_18_18_0C_18_18_30_00, // 125 '}'
    0x00_00_31_7B_4E_00_00_00, // 126 '~'
];

/// Atlas is 16 columns x 6 rows of 8x8 chars = 128x48 pixels.
const ATLAS_COLS: u32 = 16;
const ATLAS_WIDTH: u32 = 128;
const ATLAS_HEIGHT: u32 = 48;

/// Create the font atlas texture (R8Unorm, 128x48).
pub fn create_font_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView) {
    let mut pixels = vec![0u8; (ATLAS_WIDTH * ATLAS_HEIGHT) as usize];

    for (idx, &bits) in FONT_BITMAP.iter().enumerate() {
        let col = (idx as u32) % ATLAS_COLS;
        let row = (idx as u32) / ATLAS_COLS;
        let base_x = col * 8;
        let base_y = row * 8;

        for y in 0..8u32 {
            // Extract the byte for this row — top row is the most-significant byte
            let row_byte = ((bits >> (56 - y * 8)) & 0xFF) as u8;
            for x in 0..8u32 {
                let bit = (row_byte >> (7 - x)) & 1;
                let px = base_x + x;
                let py = base_y + y;
                pixels[(py * ATLAS_WIDTH + px) as usize] = if bit != 0 { 255 } else { 0 };
            }
        }
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("font atlas"),
        size: wgpu::Extent3d {
            width: ATLAS_WIDTH,
            height: ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::R8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(ATLAS_WIDTH),
            rows_per_image: Some(ATLAS_HEIGHT),
        },
        wgpu::Extent3d {
            width: ATLAS_WIDTH,
            height: ATLAS_HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Build label quads for all visible nodes.
/// Returns an empty vec when zoom < 0.5 (labels hidden when zoomed out).
pub fn build_label_quads(nodes: &[SceneNode], zoom: f32) -> Vec<LabelVertex> {
    if zoom < 0.5 {
        return Vec::new();
    }

    let char_scale = 1.0 / zoom.sqrt();
    let char_w = 0.4 * char_scale;
    let char_h = 0.5 * char_scale;

    let atlas_w = ATLAS_WIDTH as f32;
    let atlas_h = ATLAS_HEIGHT as f32;
    let cell_u = 8.0 / atlas_w;
    let cell_v = 8.0 / atlas_h;

    let mut verts = Vec::new();

    for node in nodes {
        // Extract filename from node id (last component after '/')
        let filename = node.id.rsplit('/').next().unwrap_or(&node.id);

        // Center text below node
        let total_width = filename.len() as f32 * char_w;
        let start_x = node.pos.x - total_width * 0.5;
        let start_y = node.pos.y - node.radius - char_h * 1.5;

        for (ci, ch) in filename.chars().enumerate() {
            let code = ch as u32;
            if code < 32 || code > 126 {
                continue;
            }
            let idx = code - 32;
            let atlas_col = idx % ATLAS_COLS;
            let atlas_row = idx / ATLAS_COLS;

            let u0 = atlas_col as f32 * cell_u;
            let v0 = atlas_row as f32 * cell_v;
            let u1 = u0 + cell_u;
            let v1 = v0 + cell_v;

            let x0 = start_x + ci as f32 * char_w;
            let x1 = x0 + char_w;
            let y0 = start_y;
            let y1 = start_y + char_h;

            // Two triangles (6 vertices), CCW
            verts.push(LabelVertex { position: [x0, y0], uv: [u0, v1] });
            verts.push(LabelVertex { position: [x1, y0], uv: [u1, v1] });
            verts.push(LabelVertex { position: [x1, y1], uv: [u1, v0] });

            verts.push(LabelVertex { position: [x0, y0], uv: [u0, v1] });
            verts.push(LabelVertex { position: [x1, y1], uv: [u1, v0] });
            verts.push(LabelVertex { position: [x0, y1], uv: [u0, v0] });
        }
    }

    verts
}
