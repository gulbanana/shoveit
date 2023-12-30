set signtool="C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe"
set tar="C:\Windows\System32\tar.exe"
set certificate="a2da0d655fd327e046f5878a11053f2b9c2e2233"

cargo build --release --target x86_64-pc-windows-msvc
rmdir /s /q target\windows\
mkdir target\windows\assets\
copy target\x86_64-pc-windows-msvc\release\shoveit.exe target\windows\
copy assets target\windows\assets\
%signtool% sign /fd certHash /sha1 %certificate% target\windows\shoveit.exe
%tar% -c --format zip -f "target\Shove it!.zip" -C "target\windows" *
move "target\Shove it!.zip" "target\windows\"