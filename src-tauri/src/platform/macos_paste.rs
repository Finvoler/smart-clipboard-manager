//! Send copy and paste commands to the intended Mac application.

use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use core_foundation::{
    array::CFArray,
    base::{CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use core_graphics::{
    event::{CGEvent, CGEventFlags, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSPasteboard, NSWorkspace};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementSetMessagingTimeout(element: CFTypeRef, timeout: f32) -> i32;
    fn AXUIElementCopyAttributeValue(
        element: CFTypeRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementPerformAction(element: CFTypeRef, action: CFStringRef) -> i32;
    fn CGPreflightPostEventAccess() -> bool;
}

static EXPLAINED_PERMISSION: AtomicBool = AtomicBool::new(false);
pub(super) fn paste_when_focused(target_pid: i32, pasteboard_change: isize) {
    if !unsafe { AXIsProcessTrusted() } {
        if !EXPLAINED_PERMISSION.swap(true, Ordering::Relaxed) {
            let prompt = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
            let options = CFDictionary::from_CFType_pairs(&[(prompt, CFBoolean::true_value())]);
            unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
        }
        return;
    }

    thread::spawn(move || {
        for _ in 0..12 {
            thread::sleep(Duration::from_millis(35));
            let (frontmost, clipboard_matches) = autoreleasepool(|_| {
                (
                    frontmost_pid(),
                    NSPasteboard::generalPasteboard().changeCount() == pasteboard_change,
                )
            });
            if !clipboard_matches {
                return;
            }
            if frontmost == Some(target_pid) {
                autoreleasepool(|_| {
                    paste(target_pid, pasteboard_change);
                });
                return;
            }
        }
    });
}

fn frontmost_pid() -> Option<i32> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .map(|application| application.processIdentifier())
}

fn paste(pid: i32, pasteboard_change: isize) {
    if frontmost_pid() != Some(pid) {
        return;
    }
    // Invoke the target's edit command without triggering the panel's global shortcut.
    let application = unsafe { AXUIElementCreateApplication(pid) };
    let application = (!application.is_null())
        .then(|| unsafe { CFType::wrap_under_create_rule(application) });
    let menu_item = application.and_then(|application| {
        unsafe { AXUIElementSetMessagingTimeout(application.as_CFTypeRef(), 0.15) };
        let menu_bar = attribute(&application, "AXMenuBar")?;
        let edit = children(&menu_bar)
            .into_iter()
            .find(|item| has_title(item, &["Edit", "编辑", "編輯"]))?;
        children(&edit)
            .iter()
            .flat_map(children)
            .find(|item| has_title(item, &["Paste", "粘贴", "貼上", "贴上", "黏貼"]))
    });
    if frontmost_pid() != Some(pid)
        || NSPasteboard::generalPasteboard().changeCount() != pasteboard_change
    {
        return;
    }
    if let Some(item) = menu_item {
        let press = CFString::new("AXPress");
        unsafe { AXUIElementPerformAction(item.as_CFTypeRef(), press.as_concrete_TypeRef()) };
        return;
    }

    // Apps without an accessible Edit menu can still handle standard edit keys.
    // Post them to the chosen application instead of sending them globally.
    if unsafe { CGPreflightPostEventAccess() } {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::Private) {
            for down in [true, false] {
                if let Ok(event) = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, down) {
                    event.set_flags(CGEventFlags::CGEventFlagCommand);
                    event.post_to_pid(pid);
                }
            }
        }
    }
}

fn attribute(element: &CFType, name: &str) -> Option<CFType> {
    let attribute = CFString::new(name);
    let mut value = ptr::null();
    let status = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            &mut value,
        )
    };
    (status == 0 && !value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

fn children(element: &CFType) -> Vec<CFType> {
    let Some(array) =
        attribute(element, "AXChildren").and_then(|value| value.downcast_into::<CFArray>())
    else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|child| {
            let reference = *child as *const c_void;
            (!reference.is_null()).then(|| unsafe { CFType::wrap_under_get_rule(reference) })
        })
        .collect()
}

fn has_title(element: &CFType, titles: &[&str]) -> bool {
    attribute(element, "AXTitle")
        .and_then(|value| value.downcast_into::<CFString>())
        .is_some_and(|title| titles.contains(&title.to_string().as_str()))
}
