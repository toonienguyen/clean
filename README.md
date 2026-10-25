# Toonie Clean RAM

Công cụ dọn RAM nhỏ gọn viết bằng Rust + Win32 thuần, chạy trên Windows 7 / Server 2012 trở lên.

> **Trạng thái:** mã nguồn CHƯA được biên dịch hay chạy thử. Chưa có kết quả kiểm thử thực tế nào. Xem mục "Kiểm thử" bên dưới để tự kiểm tra.

## Giao diện

- Bộ chọn chu kỳ: 5 / 10 / 15 / 30 / 60 giây, 5 / 10 / 15 / 30 phút.
- Hiển thị `RAM: x.xx GB / y.yy GB` và `RAM Usage: z%`, tự cập nhật mỗi 1 giây (độc lập với Clean/Stop).
- Một nút duy nhất: **Clean** / **Stop**.

## Cách hoạt động

- Bấm **Clean**: bắt đầu đếm theo chu kỳ đã chọn, KHÔNG dọn ngay. Hết giờ thì dọn đúng 1 lần, dọn xong mới đếm chu kỳ kế tiếp.
- Bấm **Stop**: ngăn các lần dọn sau. Nếu một lần dọn đang chạy thì để nó xong, không hủy giữa chừng.
- Khi đang chạy, bộ chọn chu kỳ bị khóa. Muốn đổi thì Stop trước, chọn lại, rồi Clean.
- Dọn lỗi (ví dụ thiếu quyền): dừng hẳn, báo lỗi một lần, nút về "Clean".
- Đọc RAM lỗi: hiển thị `--`, không hiển thị số giả.

### Thao tác dọn (mỗi lần)

1. Bật privilege `SeProfileSingleProcessPrivilege` (bắt buộc) và `SeDebugPrivilege` (tùy chọn).
2. `EmptyWorkingSet` cho từng tiến trình mở được.
3. Xóa standby list bằng `NtSetSystemInformation` (SystemMemoryListInformation, MemoryPurgeStandbyList).

**Lưu ý:** cơ chế này CHƯA được đối chiếu với công cụ cũ (CleanRam.dll, tham số "workingsets"). Đây là cơ chế phổ biến, không phải bản sao đã xác minh. Chương trình không đảm bảo giải phóng toàn bộ RAM hay cải thiện hiệu năng. Chọn chu kỳ quá ngắn có thể làm máy chậm hơn vì dữ liệu phải nạp lại từ đĩa.

## Yêu cầu build

- Windows, Rust (rustup) toolchain MSVC.
- Visual Studio Build Tools (workload "Desktop development with C++") kèm Windows SDK (cần `rc.exe` để nhúng icon và manifest).
- Cần internet lần đầu để tải crate `embed-resource` (chỉ dùng lúc build).

## Build

Bản chạy được Windows 7 / Server 2012 (khuyến nghị, target x64):

```
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
cargo +nightly build --release -Z build-std=std,panic_abort --target x86_64-win7-windows-msvc
```

EXE: `target\x86_64-win7-windows-msvc\release\clean_ram.exe`

Bản x86 (32-bit): thay target bằng `i686-win7-windows-msvc`.

### Build bằng GitHub Actions

Đẩy cả thư mục này lên repo GitHub (nhánh `main`), vào tab **Actions** → **Build EXE** → **Run workflow** (hoặc tự chạy khi push). EXE nằm ở mục **Artifacts** cuối trang run: `ToonieCleanRam-win7-x64`, `ToonieCleanRam-win7-x86`, `ToonieCleanRam-win10-x64`. Push tag dạng `v1.0.0` thì EXE được đính kèm vào Release. Workflow chưa được chạy thử.

Nếu chỉ cần chạy Windows 10 / Server 2016 trở lên, build thường:

```
cargo build --release
```

Các target `*-win7-*` là tier 3, tên và cách build có thể thay đổi theo phiên bản Rust. Chưa xác nhận trên máy thật.

## Phụ thuộc

- Runtime: không. Chỉ dùng DLL hệ thống (kernel32, user32, gdi32, advapi32, psapi, ntdll). Không cần .NET hay Java.
- Lúc build: crate `embed-resource` (build-dependency), không nằm trong EXE.

## Cấu trúc

```
clean_ram/
├── Cargo.toml
├── build.rs            nhúng tài nguyên
├── README.md
├── .github/workflows/build-exe.yml
├── src/main.rs
└── res/
    ├── app.rc          khai báo icon + manifest
    ├── app.manifest    requireAdministrator
    └── icon.ico        icon (16 đến 256 px), làm từ ảnh Toonie
```

## Kiểm thử (bạn cần tự chạy, chưa có kết quả)

- [ ] Số RAM tự cập nhật mỗi giây khi mở, khi Clean, khi Stop.
- [ ] % RAM khớp Task Manager (cùng GlobalMemoryStatusEx).
- [ ] Chọn 5 giây: không dọn ngay khi bấm Clean; đủ 5 giây thì dọn đúng 1 lần, rồi đếm lại.
- [ ] Bấm Stop: không còn lần dọn nào sau đó. Bấm Clean/Stop liên tục: không có timer trùng.
- [ ] Chạy không quyền admin / thiếu privilege: báo lỗi rõ ràng.
- [ ] Manifest: `mt.exe -inputresource:clean_ram.exe;#1 -out:out.xml`
- [ ] API import: `dumpbin /imports clean_ram.exe`
- [ ] CPU/RAM của chính app khi chờ (Task Manager), đóng app không còn tiến trình phụ.
- [ ] Chạy thực tế trên Windows Server 2012 / Windows 7.
