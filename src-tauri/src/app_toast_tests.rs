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
