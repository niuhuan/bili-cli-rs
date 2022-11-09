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

1. 将ffmpeg命令添加到PATH (使用本软件的必要条件, 合并使用)
2. 运行本软件

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
./bili-cli down https://space.bilibili.com/273715/channel/collectiondetail?sid=44375&ctype=0

```

## 已知问题

官方token有效期只有一个月

