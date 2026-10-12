//! Synthetic runs with known colours and sizes: used by tests, the benchmark and the
//! `demo` command. The fictional app is "Pocket Garden" (`com.example.pocketgarden`).

use a11y_report_android::manifest::{ManifestDevice, ManifestScreen, RunManifest};
use a11y_report_core::Rect;
use image::ImageEncoder;
use std::fs;
use std::io;
use std::path::Path;

pub const PACKAGE: &str = "com.example.pocketgarden";
pub const WIDTH: u32 = 1080;
pub const HEIGHT: u32 = 2400;
pub const DPI: u32 = 420;

type Rgb = [u8; 3];
const WHITE: Rgb = [0xFF, 0xFF, 0xFF];
const GREEN: Rgb = [0x2E, 0x7D, 0x32];
const INK: Rgb = [0x1B, 0x1B, 0x1B];
const GREY_77: Rgb = [0x77, 0x77, 0x77];
const GREY_94: Rgb = [0x94, 0x94, 0x94];
const GREY_9E: Rgb = [0x9E, 0x9E, 0x9E];
const FIELD: Rgb = [0xF1, 0xF3, 0xF4];

/// dp → px at 420 dpi.
pub fn dp(v: f64) -> i32 {
    (v * f64::from(DPI) / 160.0).round() as i32
}

/// RGB canvas with the drawing primitives used by fixtures.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Canvas {
    pub fn new(width: u32, height: u32, bg: Rgb) -> Canvas {
        Canvas {
            width,
            height,
            data: (0..width as usize * height as usize)
                .flat_map(|_| bg)
                .collect(),
        }
    }

    pub fn fill(&mut self, r: Rect, c: Rgb) {
        let r = r.clamp_to(self.width, self.height);
        for y in r.y1..r.y2 {
            let row = y as usize * self.width as usize;
            for x in r.x1..r.x2 {
                let i = (row + x as usize) * 3;
                self.data[i..i + 3].copy_from_slice(&c);
            }
        }
    }

    /// Vertical stripes 3 px wide: a busy background.
    pub fn stripes(&mut self, r: Rect, a: Rgb, b: Rgb) {
        let mut x = r.x1;
        let mut flip = false;
        while x < r.x2 {
            self.fill(
                Rect::new(x, r.y1, (x + 3).min(r.x2), r.y2),
                if flip { b } else { a },
            );
            flip = !flip;
            x += 3;
        }
    }

    /// Draws blocky pseudo-glyphs (no anti-aliasing) whose ink height is `ink_px`,
    /// vertically centred in `r`, starting 12 px from its left edge.
    pub fn text(&mut self, r: Rect, text: &str, fg: Rgb, ink_px: i32) {
        let stroke = (ink_px / 7).max(2);
        let cell = (ink_px * 55 / 100).max(stroke * 3);
        let gap = (ink_px * 18 / 100).max(2);
        let top = r.y1 + (r.height() - ink_px) / 2;
        let mut x = r.x1 + 12;
        for ch in text.chars() {
            if x + cell > r.x2 - 4 {
                break;
            }
            if !ch.is_whitespace() {
                let code = ch as u32;
                // Left stem, plus a right stem / top bar / middle bar depending on the char.
                self.fill(Rect::new(x, top, x + stroke, top + ink_px), fg);
                if code & 1 == 1 {
                    self.fill(
                        Rect::new(x + cell - stroke, top, x + cell, top + ink_px),
                        fg,
                    );
                }
                if code & 2 == 2 {
                    self.fill(Rect::new(x, top, x + cell, top + stroke), fg);
                }
                if code & 4 == 4 {
                    let mid = top + ink_px / 2 - stroke / 2;
                    self.fill(Rect::new(x, mid, x + cell, mid + stroke), fg);
                }
                self.fill(
                    Rect::new(x, top + ink_px - stroke, x + cell, top + ink_px),
                    fg,
                );
            }
            x += cell + gap;
        }
    }

    pub fn to_png(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new_with_quality(
            &mut buf,
            image::codecs::png::CompressionType::Fast,
            image::codecs::png::FilterType::Sub,
        )
        .write_image(
            &self.data,
            self.width,
            self.height,
            image::ExtendedColorType::Rgb8,
        )
        .map_err(io::Error::other)?;
        Ok(buf)
    }
}

/// Ink height in px for a font size in sp (ink ≈ 0.9 × font size).
fn ink(sp: f64) -> i32 {
    dp(sp * 0.9)
}

/// Minimal uiautomator node.
#[derive(Default, Clone)]
pub struct N {
    pub class: &'static str,
    pub rid: Option<String>,
    pub text: Option<String>,
    pub desc: Option<String>,
    pub bounds: Rect,
    pub clickable: bool,
    pub focusable: bool,
    pub checkable: bool,
    pub scrollable: bool,
    pub disabled: bool,
    pub children: Vec<N>,
}

impl N {
    pub fn new(class: &'static str, bounds: Rect) -> N {
        N {
            class,
            bounds,
            ..N::default()
        }
    }
    pub fn id(mut self, rid: &str) -> N {
        self.rid = Some(format!("{PACKAGE}:id/{rid}"));
        self
    }
    pub fn text(mut self, t: &str) -> N {
        self.text = Some(t.into());
        self
    }
    pub fn desc(mut self, d: &str) -> N {
        self.desc = Some(d.into());
        self
    }
    pub fn clickable(mut self) -> N {
        self.clickable = true;
        self.focusable = true;
        self
    }
    pub fn checkable(mut self) -> N {
        self.checkable = true;
        self
    }
    pub fn scrollable(mut self) -> N {
        self.scrollable = true;
        self
    }
    pub fn child(mut self, c: N) -> N {
        self.children.push(c);
        self
    }

    fn write(&self, index: usize, depth: usize, out: &mut String) {
        let esc = |s: &str| {
            s.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
        };
        let b = self.bounds;
        out.push_str(&"  ".repeat(depth + 1));
        out.push_str(&format!(
            "<node index=\"{index}\" text=\"{}\" resource-id=\"{}\" class=\"{}\" package=\"{PACKAGE}\" content-desc=\"{}\" checkable=\"{}\" checked=\"false\" clickable=\"{}\" enabled=\"{}\" focusable=\"{}\" focused=\"false\" scrollable=\"{}\" long-clickable=\"false\" password=\"false\" selected=\"false\" bounds=\"[{},{}][{},{}]\"",
            esc(self.text.as_deref().unwrap_or("")),
            esc(self.rid.as_deref().unwrap_or("")),
            self.class,
            esc(self.desc.as_deref().unwrap_or("")),
            self.checkable,
            self.clickable,
            !self.disabled,
            self.focusable,
            self.scrollable,
            b.x1, b.y1, b.x2, b.y2
        ));
        if self.children.is_empty() {
            out.push_str(" />\n");
        } else {
            out.push_str(">\n");
            for (i, c) in self.children.iter().enumerate() {
                c.write(i, depth + 1, out);
            }
            out.push_str(&"  ".repeat(depth + 1));
            out.push_str("</node>\n");
        }
    }
}

pub fn hierarchy_xml(root: &N) -> String {
    let mut out = String::from(
        "<?xml version='1.0' encoding='UTF-8' standalone='yes' ?>\n<hierarchy rotation=\"0\">\n",
    );
    root.write(0, 0, &mut out);
    out.push_str("</hierarchy>\n");
    out
}

pub struct SynthScreen {
    pub name: String,
    pub activity: Option<String>,
    pub xml: String,
    pub png: Option<Vec<u8>>,
}

const W: i32 = WIDTH as i32;
const H: i32 = HEIGHT as i32;

fn home() -> io::Result<SynthScreen> {
    let mut c = Canvas::new(WIDTH, HEIGHT, WHITE);
    let toolbar = Rect::new(0, 0, W, dp(56.0));
    c.fill(toolbar, GREEN);
    let menu = Rect::new(0, 0, dp(56.0), dp(56.0));
    c.fill(Rect::new(dp(16.0), dp(18.0), dp(40.0), dp(38.0)), WHITE);
    let title = Rect::new(dp(72.0), dp(8.0), dp(300.0), dp(48.0));
    c.text(title, "Pocket Garden", WHITE, ink(20.0));
    let search = Rect::new(dp(16.0), dp(68.0), W - dp(16.0), dp(116.0));
    c.fill(search, FIELD);

    let list = Rect::new(0, dp(126.0), W, dp(820.0));
    let mut recycler = N::new("androidx.recyclerview.widget.RecyclerView", list)
        .id("plant_list")
        .scrollable();
    for (i, plant) in ["Monstera", "Calathea", "Pothos"].iter().enumerate() {
        let y0 = dp(136.0) + i as i32 * dp(160.0);
        let card = Rect::new(dp(16.0), y0, W - dp(16.0), y0 + dp(148.0));
        let name = Rect::new(dp(28.0), y0 + dp(12.0), dp(260.0), y0 + dp(44.0));
        let hint = Rect::new(dp(28.0), y0 + dp(50.0), dp(300.0), y0 + dp(78.0));
        let water = Rect::new(W - dp(140.0), y0 + dp(96.0), W - dp(28.0), y0 + dp(134.0));
        c.text(name, plant, INK, ink(18.0));
        c.text(hint, "Water every 7 days", GREY_77, ink(14.0));
        c.fill(water, GREEN);
        c.text(water, "Water", WHITE, ink(14.0));
        recycler = recycler.child(
            N::new("android.widget.LinearLayout", card)
                .id("plant_card")
                .child(
                    N::new("android.widget.TextView", name)
                        .id("plant_name")
                        .text(plant),
                )
                .child(N::new("android.widget.TextView", hint).text("Water every 7 days"))
                .child(
                    N::new("android.widget.Button", water)
                        .id("btn_water")
                        .text("Water")
                        .clickable(),
                ),
        );
    }

    let banner = Rect::new(0, dp(830.0), dp(340.0), H);
    c.stripes(banner, [0xC8, 0xE6, 0xC9], [0x81, 0xC7, 0x84]);
    let banner_text = Rect::new(dp(8.0), dp(840.0), dp(330.0), dp(890.0));
    c.text(banner_text, "Seasonal tips", WHITE, ink(16.0));
    let tips = Rect::new(W - dp(40.0), dp(850.0), W - dp(21.0), dp(869.0));
    c.fill(tips, GREEN);

    let root = N::new("android.widget.FrameLayout", Rect::new(0, 0, W, H))
        .child(
            N::new("android.widget.LinearLayout", toolbar)
                .id("toolbar")
                .child(
                    N::new("android.widget.ImageButton", menu)
                        .id("menu")
                        .clickable(),
                )
                .child(N::new("android.widget.TextView", title).text("Pocket Garden")),
        )
        .child(
            N::new("android.widget.EditText", search)
                .id("search_box")
                .clickable(),
        )
        .child(recycler)
        .child(
            N::new("android.widget.TextView", banner_text)
                .id("seasonal_banner")
                .text("Seasonal tips"),
        )
        .child(
            N::new("android.widget.ImageButton", tips)
                .id("btn_tips")
                .desc("Plant care tips")
                .clickable(),
        );
    Ok(SynthScreen {
        name: "home".into(),
        activity: Some(format!("{PACKAGE}/.ui.HomeActivity")),
        xml: hierarchy_xml(&root),
        png: Some(c.to_png()?),
    })
}

fn plant_detail() -> io::Result<SynthScreen> {
    let mut c = Canvas::new(WIDTH, HEIGHT, WHITE);
    let back = Rect::new(0, 0, dp(56.0), dp(56.0));
    c.fill(Rect::new(dp(16.0), dp(18.0), dp(40.0), dp(38.0)), INK);
    let title = Rect::new(dp(16.0), dp(72.0), W - dp(16.0), dp(130.0));
    c.text(title, "Monstera deliciosa", GREY_94, ink(30.0));
    let light = Rect::new(dp(16.0), dp(140.0), W - dp(16.0), dp(172.0));
    c.text(light, "Light: bright, indirect", GREY_9E, ink(14.0));
    let reminders = Rect::new(dp(16.0), dp(190.0), W - dp(16.0), dp(246.0));
    c.text(reminders, "Reminders", INK, ink(16.0));
    let share = Rect::new(dp(16.0), dp(260.0), dp(200.0), dp(316.0));
    let share_label = Rect::new(dp(20.0), dp(266.0), dp(190.0), dp(310.0));
    c.text(share_label, "Share", INK, ink(16.0));
    let delete = Rect::new(dp(16.0), dp(340.0), dp(130.0), dp(362.0));
    c.fill(delete, [0xB7, 0x1C, 0x1C]);
    c.text(
        Rect::new(dp(16.0), dp(340.0), dp(130.0), dp(362.0)),
        "Delete",
        WHITE,
        ink(10.0),
    );
    let add = Rect::new(W - dp(72.0), H - dp(72.0), W - dp(16.0), H - dp(16.0));
    c.fill(add, GREEN);

    let root = N::new("android.widget.FrameLayout", Rect::new(0, 0, W, H))
        .child(
            N::new("android.widget.ImageButton", back)
                .desc("Back")
                .clickable(),
        )
        .child(
            N::new("android.widget.TextView", title)
                .id("plant_title")
                .text("Monstera deliciosa"),
        )
        .child(
            N::new("android.widget.TextView", light)
                .id("plant_light")
                .text("Light: bright, indirect"),
        )
        .child(
            N::new("android.widget.Switch", reminders)
                .id("switch_reminders")
                .text("Reminders")
                .clickable()
                .checkable(),
        )
        .child(
            N::new("android.widget.LinearLayout", share)
                .id("row_share")
                .clickable()
                .child(N::new("android.widget.TextView", share_label).text("Share")),
        )
        .child(
            N::new("android.widget.Button", delete)
                .id("btn_delete")
                .text("Delete")
                .clickable(),
        )
        .child(N::new("android.view.View", add).clickable());
    Ok(SynthScreen {
        name: "plant_detail".into(),
        activity: Some(format!("{PACKAGE}/.ui.PlantDetailActivity")),
        xml: hierarchy_xml(&root),
        png: Some(c.to_png()?),
    })
}

fn settings() -> io::Result<SynthScreen> {
    let mut c = Canvas::new(WIDTH, HEIGHT, WHITE);
    let back = Rect::new(0, 0, dp(56.0), dp(56.0));
    c.fill(Rect::new(dp(16.0), dp(18.0), dp(40.0), dp(38.0)), INK);
    let mut root = N::new("android.widget.FrameLayout", Rect::new(0, 0, W, H)).child(
        N::new("android.widget.ImageButton", back)
            .desc("Back")
            .clickable(),
    );
    for (i, label) in ["Notifications", "Dark theme", "Metric units"]
        .iter()
        .enumerate()
    {
        let y0 = dp(80.0) + i as i32 * dp(64.0);
        let row = Rect::new(dp(16.0), y0, W - dp(16.0), y0 + dp(56.0));
        c.text(row, label, INK, ink(16.0));
        root = root.child(
            N::new("android.widget.Switch", row)
                .id(&format!("switch_{i}"))
                .text(label)
                .clickable()
                .checkable(),
        );
    }
    Ok(SynthScreen {
        name: "settings".into(),
        activity: Some(format!("{PACKAGE}/.ui.SettingsActivity")),
        xml: hierarchy_xml(&root),
        png: Some(c.to_png()?),
    })
}

/// The three demo screens of Pocket Garden.
pub fn demo_screens() -> io::Result<Vec<SynthScreen>> {
    Ok(vec![home()?, plant_detail()?, settings()?])
}

pub fn demo_manifest(screens: &[SynthScreen], build: &str) -> RunManifest {
    RunManifest {
        schema: 1,
        app: "pocket-garden".into(),
        platform: "android".into(),
        build: build.into(),
        captured_at: "2026-10-12T09:30:00Z".into(),
        package: Some(PACKAGE.into()),
        device: ManifestDevice {
            model: "Pixel 7".into(),
            density_dpi: Some(DPI),
            width_px: WIDTH,
            height_px: HEIGHT,
            font_scale: 1.0,
        },
        screens: screens
            .iter()
            .map(|s| ManifestScreen {
                name: s.name.clone(),
                hierarchy: format!("screens/{}.xml", s.name),
                screenshot: s.png.as_ref().map(|_| format!("screens/{}.png", s.name)),
                activity: s.activity.clone(),
            })
            .collect(),
    }
}

/// Writes `run.json` and `screens/*` into `dir`.
pub fn write_run(dir: &Path, screens: &[SynthScreen], build: &str) -> io::Result<()> {
    fs::create_dir_all(dir.join("screens"))?;
    for s in screens {
        fs::write(dir.join(format!("screens/{}.xml", s.name)), &s.xml)?;
        if let Some(png) = &s.png {
            fs::write(dir.join(format!("screens/{}.png", s.name)), png)?;
        }
    }
    fs::write(
        dir.join("run.json"),
        demo_manifest(screens, build).to_json(),
    )
}

/// `count` screens built by cycling through the demo screens (benchmark input).
pub fn write_bench_run(dir: &Path, count: usize) -> io::Result<()> {
    let base = demo_screens()?;
    let screens: Vec<SynthScreen> = (0..count)
        .map(|i| {
            let b = &base[i % base.len()];
            SynthScreen {
                name: format!("screen_{i:03}"),
                activity: b.activity.clone(),
                xml: b.xml.clone(),
                png: b.png.clone(),
            }
        })
        .collect();
    write_run(dir, &screens, "1.0.0 (1)")
}

/// A tiny fictional Android project matching the demo screens, so source hints resolve.
pub fn write_demo_project(root: &Path) -> io::Result<()> {
    let files: [(&str, &str); 6] = [
        (
            "settings.gradle.kts",
            "rootProject.name = \"pocket-garden\"\ninclude(\":app\")\n",
        ),
        (
            "app/build.gradle.kts",
            "plugins {\n    id(\"com.android.application\")\n    id(\"org.jetbrains.kotlin.android\")\n}\n\nandroid {\n    namespace = \"com.example.pocketgarden\"\n    defaultConfig {\n        applicationId = \"com.example.pocketgarden\"\n        versionCode = 240\n        versionName = \"2.4.0\"\n    }\n}\n",
        ),
        (
            "app/src/main/res/values/strings.xml",
            "<resources>\n    <string name=\"app_name\">Pocket Garden</string>\n    <string name=\"water\">Water</string>\n    <string name=\"water_hint\">Water every 7 days</string>\n    <string name=\"seasonal_tips\">Seasonal tips</string>\n    <string name=\"light_hint\">Light: bright, indirect</string>\n</resources>\n",
        ),
        (
            "app/src/main/res/layout/item_plant.xml",
            "<LinearLayout xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:id=\"@+id/plant_card\"\n    android:layout_width=\"match_parent\"\n    android:layout_height=\"wrap_content\">\n\n    <TextView\n        android:id=\"@+id/plant_name\"\n        android:textColor=\"@color/ink\" />\n\n    <TextView\n        android:text=\"@string/water_hint\"\n        android:textColor=\"#777777\" />\n\n    <Button\n        android:id=\"@+id/btn_water\"\n        android:text=\"@string/water\"\n        android:minHeight=\"38dp\" />\n</LinearLayout>\n",
        ),
        (
            "app/src/main/res/layout/activity_home.xml",
            "<FrameLayout xmlns:android=\"http://schemas.android.com/apk/res/android\">\n    <ImageButton\n        android:id=\"@+id/menu\"\n        android:src=\"@drawable/ic_menu\" />\n    <EditText\n        android:id=\"@+id/search_box\" />\n    <TextView\n        android:id=\"@+id/seasonal_banner\"\n        android:text=\"@string/seasonal_tips\" />\n</FrameLayout>\n",
        ),
        (
            "app/src/main/java/com/example/pocketgarden/ui/PlantDetailScreen.kt",
            "package com.example.pocketgarden.ui\n\n@Composable\nfun PlantDetailScreen(plant: Plant) {\n    Text(plant.name, color = Grey94, modifier = Modifier.testTag(\"plant_title\"))\n    Text(stringResource(R.string.light_hint), color = Grey9E)\n    Button(onClick = {}, modifier = Modifier.height(22.dp).testTag(\"btn_delete\")) { Text(\"Delete\") }\n    Box(Modifier.size(56.dp).clickable { addPlant() })\n}\n",
        ),
    ];
    for (rel, content) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
    }
    Ok(())
}
