//! The ready toast's markup, loaded the way Windows loads it.

use super::*;
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::ToastNotification;

#[test]
fn the_ready_toast_launches_its_job_and_escapes_the_title() {
    let doc = XmlDocument::new().unwrap();
    doc.LoadXml(&HSTRING::from(toast_xml("자막: a & b <c> \"d\"", "1b2c-3d"))).unwrap();
    let toast = doc.DocumentElement().unwrap();
    assert_eq!(toast.GetAttribute(&"activationType".into()).unwrap(), "protocol");
    assert_eq!(toast.GetAttribute(&"launch".into()).unwrap(), "vto-open:1b2c-3d");
    let texts = doc.GetElementsByTagName(&"text".into()).unwrap();
    assert_eq!(texts.Item(1).unwrap().InnerText().unwrap(), "자막: a & b <c> \"d\"");
    ToastNotification::CreateToastNotification(&doc).unwrap();
}

/// Shows a real toast (under PowerShell's app id, as unpackaged builds do), so opt-in.
#[test]
#[ignore = "shows a toast on this desktop"]
fn a_job_whose_toast_is_still_shown_gets_no_second_one() {
    use windows::UI::Notifications::ToastNotificationManager;
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let job = format!("test-{}", std::process::id());
    let xml = toast_xml("자막 준비 완료 테스트", &job);
    assert!(show_once(POWERSHELL_APP_ID, &xml, &job).unwrap(), "the first send shows the toast");
    assert!(!show_once(POWERSHELL_APP_ID, &xml, &job).unwrap(), "a second send while it is shown adds nothing");
    let history = ToastNotificationManager::History().unwrap();
    history.RemoveGroupedTagWithId(&job.as_str().into(), &TOAST_GROUP.into(), &POWERSHELL_APP_ID.into()).unwrap();
    assert!(show_once(POWERSHELL_APP_ID, &xml, &job).unwrap(), "once it is gone, a send shows it again");
    history.RemoveGroupedTagWithId(&job.as_str().into(), &TOAST_GROUP.into(), &POWERSHELL_APP_ID.into()).unwrap();
}
