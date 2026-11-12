---
scope: project
severity: contract
triggers: ["모바일", "mobile", "안드로이드", "android", "갤럭시", "S21", "rust", "러스트", "프레임워크", "framework", "최적화"]
slots: []
sources: [README.md]
links: [job-survives-background, project]
---

# The app is built on a Rust-based web framework and is accepted on a Galaxy S21 Ultra

Rule. Choose the web framework from the Rust ecosystem, as the README asks,
and treat the Galaxy S21 Ultra as the acceptance device: a feature that works
on desktop but has not run on that phone is not done. Ask the user before
moving the stack off Rust.

Why. `README.md` asks for a Rust-based web framework for mobile performance
and names the S21 Ultra as the test phone.

What goes wrong. A desktop-only check hides memory, battery and background
limits that only show on the phone.
