<div align="center">
  <img src="assets/brewake.svg" width="128" alt="Logo Brewake">
  <h1>Brewake</h1>
  <p><strong>Giữ máy tính luôn thức, thật đơn giản.</strong></p>

  [English](README.md) · Tiếng Việt
</div>

## Giới thiệu

Brewake là tiện ích system tray nhỏ gọn, native và mã nguồn mở dành cho Windows. Ứng dụng giúp máy không tự sleep khi tải file lớn, thuyết trình, build phần mềm, điều khiển từ xa hoặc chạy một tác vụ dài—mà không thay đổi power plan.

Brewake dùng API execution state của Windows, không giả lập phím. Ứng dụng không có WebView, dịch vụ nền, tài khoản, quảng cáo, quyền administrator, kết nối mạng, analytics hay telemetry.

## Tính năng

- Nhấp trái icon để bật/tắt nhanh; nhấp phải để mở toàn bộ menu.
- Giữ máy thức vô thời hạn hoặc theo hẹn giờ từ 15 phút đến 8 giờ.
- Tạm ngưng 15 phút đến 4 giờ rồi tự động hoạt động lại.
- Tuỳ chọn giữ cả màn hình luôn bật.
- Khởi động cùng Windows cho user hiện tại, không cần quyền admin.
- Điều khiển và đọc trạng thái bằng `brewakectl`.
- Tự áp dụng lại yêu cầu sau power event và báo kết quả API thực tế.
- Single-instance, không mạng, không theo dõi người dùng.

## Tải và chạy

1. Tải `Brewake-<phiên-bản>-windows-x64.zip` tại [GitHub Releases](https://github.com/thanhbican/brewake/releases/latest).
2. Có thể đối chiếu file với checksum `.sha256` đi kèm.
3. Giải nén rồi chạy `brewake.exe`.
4. Tìm icon chiếc cốc trong notification area; Windows có thể đặt icon dưới nút `^`.

Gói tải gồm `brewake.exe`, công cụ tuỳ chọn `brewakectl.exe`, README và giấy phép MIT.

Bản phát hành hiện chưa được code-sign nên Windows SmartScreen có thể báo “unrecognized app”. Hãy kiểm tra checksum và chỉ tải từ repository chính thức này.

## Cách dùng menu tray

- **Left-click:** bật/tắt trạng thái giữ thức.
- **Active:** giữ máy thức cho đến khi bạn tắt.
- **Active for:** hoạt động trong khoảng thời gian đã chọn.
- **Inactive for:** tạm dừng rồi tự bật lại.
- **Keep display awake:** không cho màn hình tự tắt.
- **Start with Windows:** chạy cùng Windows cho tài khoản hiện tại.
- **Show status:** xem chế độ, thời hạn và kết quả Windows API.
- **About Brewake:** xem phiên bản, nguyên tắc riêng tư, repository và license.
- **Exit:** giải phóng yêu cầu nguồn điện rồi thoát sạch.

## Điều khiển bằng CLI

Đặt `brewakectl.exe` cạnh `brewake.exe`:

```powershell
brewakectl status
brewakectl status --json
brewakectl on
brewakectl on --for 2h
brewakectl pause 30m
brewakectl off
brewakectl toggle
brewakectl exit
```

Thời lượng hỗ trợ hậu tố `s`, `m`, `h`, `d` và tối đa 365 ngày. Nếu app tray chưa chạy, lệnh điều khiển sẽ tự mở file `brewake.exe` nằm cạnh nó.

Khi active, Brewake gọi `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`. Tuỳ chọn giữ màn hình bổ sung `ES_DISPLAY_REQUIRED`. Khi tắt, app trả về `ES_CONTINUOUS`. Brewake không sửa power plan và không inject phím.

## Dữ liệu và riêng tư

- Config: `%LocalAppData%\Brewake\config.json`
- Trạng thái tạm: `%LocalAppData%\Brewake\runtime.json`
- Startup: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`

Mọi xử lý đều ở máy local. Brewake không có dependency mạng hoặc telemetry và không yêu cầu quyền administrator.

## Build và test từ source

Yêu cầu Windows 10/11 x64, Rust stable MSVC từ 1.85 và Visual Studio Build Tools với workload **Desktop development with C++**.

```powershell
git clone https://github.com/thanhbican/brewake.git
cd brewake
cargo build --release --locked

cargo fmt --all --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --locked
```

Test hồi quy menu tray cần desktop Windows tương tác:

```powershell
cargo test --test windows_tray_smoke --locked -- --ignored --nocapture
```

## Đóng góp và hỗ trợ

Xem [hướng dẫn đóng góp](CONTRIBUTING.md), [hỗ trợ](SUPPORT.md), [chính sách bảo mật](SECURITY.md), [quy tắc ứng xử](CODE_OF_CONDUCT.md) và [changelog](CHANGELOG.md). Bug report và pull request tập trung đều được chào đón.

## Giấy phép

[MIT](LICENSE) © Brewake contributors. Duy trì bởi [@thanhbican](https://github.com/thanhbican).
