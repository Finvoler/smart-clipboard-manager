//! On-demand OCR using the language packs installed with Windows.
//! The synchronous WinRT calls run on Tauri's blocking thread pool.

use std::path::Path;

#[cfg(windows)]
static OCR_BUSY: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(windows)]
struct Apartment;

#[cfg(windows)]
impl Apartment {
    fn new() -> Result<Self, String> {
        use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .map_err(|e| format!("Cannot initialize Windows OCR: {e}"))?;
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::WinRT::RoUninitialize() };
    }
}

// Chinese recognition also handles Latin letters; prefer it when installed.
// Otherwise use the user's preferred language, then any installed recognizer.
#[cfg(windows)]
fn create_engine() -> Result<windows::Media::Ocr::OcrEngine, String> {
    use windows::{core::HSTRING, Globalization::Language, Media::Ocr::OcrEngine};
    let languages = OcrEngine::AvailableRecognizerLanguages()
        .map_err(|e| format!("Windows OCR is unavailable: {e}"))?;
    if languages.Size().map_err(|e| e.to_string())? == 0 {
        return Err("未安装 Windows OCR 语言包。请在 Windows 设置 → 时间和语言 → 语言选项中安装所需语言的光学字符识别。No Windows OCR language pack installed.".into());
    }
    for tag in ["zh-Hans-CN", "zh-Hant-TW"] {
        if let Ok(language) = Language::CreateLanguage(&HSTRING::from(tag)) {
            if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
                return Ok(engine);
            }
        }
    }
    if let Ok(engine) = OcrEngine::TryCreateFromUserProfileLanguages() {
        return Ok(engine);
    }
    for language in languages {
        if let Ok(engine) = OcrEngine::TryCreateFromLanguage(&language) {
            return Ok(engine);
        }
    }
    Err("未安装 Windows OCR 语言包。请在 Windows 设置 → 时间和语言 → 语言选项中安装所需语言的光学字符识别。No Windows OCR language pack installed.".into())
}

#[cfg(windows)]
pub fn recognize_image(path: &Path) -> Result<String, String> {
    use windows::{
        Graphics::Imaging::{BitmapDecoder, BitmapTransform, BitmapPixelFormat, BitmapAlphaMode,
            ExifOrientationMode, ColorManagementMode},
        Media::Ocr::OcrEngine,
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
    };

    let _busy = OCR_BUSY.try_lock().map_err(|_| "OCR 正在识别，请稍后重试 / OCR is busy".to_string())?;
    let _apartment = Apartment::new()?;

    let file_path = path.to_string_lossy();
    if !path.is_file() {
        return Err(format!("OCR image does not exist: {file_path}"));
    }

    let engine = create_engine()?;
    let bytes = std::fs::read(path).map_err(|error| format!("Cannot read OCR image: {error}"))?;
    let stream = InMemoryRandomAccessStream::new()
        .map_err(|error| format!("Cannot create OCR image stream: {error}"))?;
    let writer = DataWriter::CreateDataWriter(&stream)
        .map_err(|error| format!("Cannot write OCR image stream: {error}"))?;
    writer
        .WriteBytes(&bytes)
        .and_then(|_| writer.StoreAsync()?.get().map(|_| ()))
        .map_err(|error| format!("Cannot write OCR image stream: {error}"))?;
    drop(bytes);
    writer.DetachStream().map_err(|error| error.to_string())?;
    drop(writer);
    stream
        .Seek(0)
        .map_err(|error| format!("Cannot read OCR image stream: {error}"))?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|operation| operation.get())
        .map_err(|error| format!("Cannot decode OCR image: {error}"))?;
    let width = decoder.PixelWidth().map_err(|e| e.to_string())?;
    let height = decoder.PixelHeight().map_err(|e| e.to_string())?;
    let limit = OcrEngine::MaxImageDimension().map_err(|e| e.to_string())?;
    let (width, height) = bounded_dimensions(width, height, limit);
    let transform = BitmapTransform::new().map_err(|e| e.to_string())?;
    transform.SetScaledWidth(width).map_err(|e| e.to_string())?;
    transform.SetScaledHeight(height).map_err(|e| e.to_string())?;
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore,
            &transform, ExifOrientationMode::RespectExifOrientation, ColorManagementMode::DoNotColorManage)
        .and_then(|operation| operation.get())
        .map_err(|error| format!("Cannot decode OCR bitmap: {error}"))?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|operation| operation.get())
        .map_err(|error| format!("Windows OCR failed: {error}"))?;
    let lines = result.Lines().map_err(|e| e.to_string())?;
    let mut text = Vec::new();
    for line in lines {
        text.push(join_spaced_chinese(&line.Text().map_err(|e| e.to_string())?.to_string()));
    }
    let text = text.join("\n");
    if text.trim().is_empty() {
        return Err("Windows OCR found no readable text in this image".to_string());
    }
    Ok(text)
}

#[cfg(windows)]
fn bounded_dimensions(width: u32, height: u32, limit: u32) -> (u32, u32) {
    // Keep decoded bitmap below 64 MB and respect the OS per-edge limit.
    let scale = (limit as f64 / width.max(height).max(1) as f64)
        .min((16_000_000.0 / (width as f64 * height as f64).max(1.0)).sqrt())
        .min(1.0);
    (((width as f64 * scale) as u32).max(1), ((height as f64 * scale) as u32).max(1))
}

#[cfg(windows)]
fn join_spaced_chinese(text: &str) -> String {
    fn is_han(c: char) -> bool {
        matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
    }

    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut previous = None;
    while let Some(c) = chars.next() {
        if c == ' ' {
            let next_han = chars.peek().is_some_and(|next| is_han(*next));
            let previous_han = previous.is_some_and(is_han);
            if previous_han && next_han {
                continue;
            }
        }
        output.push(c);
        previous = Some(c);
    }
    output
}

#[cfg(not(windows))]
pub fn recognize_image(_path: &Path) -> Result<String, String> {
    Err("Local OCR requires Windows".to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn bounds_large_images_without_upscaling() {
        assert_eq!(bounded_dimensions(1100, 200, 10000), (1100, 200));
        assert_eq!(bounded_dimensions(20000, 1000, 10000), (10000, 500));
        assert_eq!(bounded_dimensions(10000, 10000, 10000), (4000, 4000));
    }

    #[test]
    #[ignore = "requires the Windows Simplified Chinese OCR language pack"]
    fn recognizes_chinese_and_english_without_api_key() {
        let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ocr-smoke.png");
        let text = recognize_image(&image).expect("Windows OCR should recognize the fixture");
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(compact.contains("剪贴板OCR测试123ABC"), "unexpected OCR text: {text}");
        assert!(text.lines().count() >= 2, "OCR must preserve line breaks");
    }

    #[test]
    fn joins_spaced_chinese_without_changing_word_spaces() {
        assert_eq!(join_spaced_chinese("剪 贴 板 OCR 测 试 123 ABC"), "剪贴板 OCR 测试 123 ABC");
    }
}
