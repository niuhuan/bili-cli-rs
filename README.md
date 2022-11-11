bili-cli
=========

哔哩哔哩视频命令行下载器

## 用户界面

![](images/down_bv.gif)

## 实现功能

- [x] 用户
  - [x] 登录
  - [x] 个人信息
- [x] 视频下载
  - [x] 高清视频下载并合并
  - [x] BV下载
  - [x] EP/SS下载
  - [x] 来自手机的短视频/短链接
  - [x] 断点续传
    - [x] dash模式 (-r 参数)
  - [x] 集合下载时选择EP

## 如何使用

```shell

###  登录线管

# 登录你的账号
./bili-cli login

# 在控制台打印二维码
./bili-cli login -c

# 登录后显示自己的信息

./bili-cli user

### 下载相关

# 打印下载帮助
./bili-cli down -h

# 下载BV （ID或者URL）
./bili-cli down BV1814y1p7Uj
./bili-cli down https://www.bilibili.com/video/BV1W44y1Y7mQ/?spm_id_from=333.999.0.0

# 下载合集或番剧 (随便找一集，把url贴进去，会下载这个动漫的所有季，所有集，并放好文件夹)
./bili-cli down https://www.bilibili.com/bangumi/play/ss4188?spm_id_from=333.337.0.0
# --choose-seasons 加上可以选择下载哪一季
# --resume 失败时断点续传

# 下载用户的合集 （合集的页面的url，会将这个合集下载到一个文件夹）
./bili-cli down "https://space.bilibili.com/273715/channel/collectiondetail?sid=44375&ctype=0"

```

## 已知问题

官方token有效期只有一个月

## 如何构建

### 构建1: 使用命令行方式调用ffmpeg

需安装ffmpeg命令行程序。

```shell
cargo build --release
```

### 构建方式2: 将ffmpegApi静态链接到bin

这种方式用户不需要额外安装ffmpeg. 但是需要在构建时链接ffmpeg依赖库。

```shell
cargo build --release --features=ffmpeg_api
```

#### 依赖库的安装

##### windows

- 安装 vcpkg
- 根据 vcpkg install ffmpeg --triplet=x64-windows-static-md
- 如果您在中国大陆的网络环境下，您可能需要设置代理之后再运行 vcpkg install 命令
  ```PowerShell
  $env:HTTP_PROXY = http://host:port/
  $env:HTTPS_PROXY = http://host:port/
  ```
##### *nix

- 使用PkgConfig

根据rusty_ffmpeg官方文档需要设置FFMPEG_PKG_CONFIG_PATH变量。

（linux构建成功，macos12构建失败，调试中）

```shell
# 克隆ffmpeg并检出release/4.4
git clone https://git.ffmpeg.org/ffmpeg.git
cd ffmpeg
git checkout release/4.4

# 构建ffmpeg并安装
mkdir build
cd build
../configure --prefix=/Volumes/DATA/Runtimes/ffmpeg4.4
make -j12
make install
```

```shell
export FFMPEG_PKG_CONFIG_PATH=/Volumes/DATA/Runtimes/ffmpeg4.4/lib/pkgconfig
cargo build --release
```
