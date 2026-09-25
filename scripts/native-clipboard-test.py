"""Test helper; only synthetic clipboard inputs. Requires pywin32 and Pillow."""
import io
import sys
import time
import win32clipboard as clip
from PIL import Image

for attempt in range(20):
    try:
        clip.OpenClipboard()
        break
    except Exception:
        time.sleep(.05)
else:
    raise RuntimeError('Clipboard remained locked')
try:
    if sys.argv[1] == 'text':
        clip.EmptyClipboard()
        clip.SetClipboardText(sys.argv[2], clip.CF_UNICODETEXT)
    elif sys.argv[1] == 'image':
        image = Image.open(sys.argv[2]).convert('RGB')
        data = io.BytesIO()
        image.save(data, 'BMP')
        clip.EmptyClipboard()
        clip.SetClipboardData(clip.CF_DIB, data.getvalue()[14:])
    elif sys.argv[1] == 'assert-text':
        assert clip.GetClipboardData(clip.CF_UNICODETEXT) == sys.argv[2]
    elif sys.argv[1] == 'assert-image':
        assert clip.IsClipboardFormatAvailable(clip.CF_DIB)
finally:
    clip.CloseClipboard()
