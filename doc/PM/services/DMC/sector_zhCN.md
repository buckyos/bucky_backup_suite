# 备份扇区格式设计

当备份任务的target是存储单元很大的写入不可修改的存储系统时（比如DMCX），首先从source生成写入扇区，之后将扇区提交到target；扇区的格式应当实现以下需求：
+ 对target的能力只做如下约束：
    + 单次写入，不可附加，不可修改
    + 可以遍历所有已写入内容
    + 可以任意偏移和大小读出
+ 扇区支持加密和校验，当target为不可信的公网节点时，为了保证隐私和安全，应当保证只有源可以从扇区还原，并且还原时可以校验内容是否正确；
+ 元数据应当嵌入到扇区中以支持零知识的本地还原；
+ 对target有较好的空间性能：
    + 支持增量备份
    + 支持备份内容压缩
    + 支持合并多次备份到一个扇区
+ 备份和还原过程应当有较好的空间性能：
    + 备份过程中产生的临时磁盘占用，相对于source的大小应当是O(1)的，较大的备份内容可以分割到多个扇区中去，顺序生成确定大小的扇区之后写入target，本地可以删除该扇区；
    + 备份过程中对备份内容加密和压缩过程是原地的，不需要额外的空间；备份和还原过程中的解密和解压缩过程也应当是原地的，不需要额外的空间；
    + 零知识还原过程可以只重建元数据，而不需要重建备份数据；


## 结构

### 整体
+ magic

    确定的4字节，target并非只用于备份，在零知识还原时，可以通过遍历target已写入内容匹配该magic确定是否为备份扇区；

+ sector header

    扇区头，无论是否加密和压缩，扇区头部分都是明文原文

+ check point boxes

    一到多个check point box；
    check point表示在某个时间点进行的一次备份，该次check point产生的备份内容被封装到check point box中；当check point比较大时，可以拆分到多个box；当有多个较小的check point时，可以合并到一个box中；


### sector header
+ version
    
    2字节版本号，用于实现扇区版本兼容性

+ flags

    32bit的开关值，当前可有
    + 加密： 是否经过加密
    + 签名： 是否包含签名

+ [public key]

    加密或签名时存在该字段，写入用于加密和签名的非对称密钥对的公钥；

+ [enrypt key]

    加密时存在该字段；
    加密过程应当首先产生非对称密钥对和对称密钥，使用对称密钥对内容加密，使用共钥对对称密钥加密后写入该字段；

+ [signature]

    签名时存在该字段，对扇区除本字段之外的所有内容做摘要之后，使用私钥对摘要加密之后写入该字段；

### check point box
+ uuid

    为每一个check point生成uuid，如果check point被分拆到多个box时，这些box有相同的uuid；
+ offset in check point

    该box在check point中的起始偏移，如果check point没有分拆，该值为0；

+ box's length
    
    box content的长度

+ box content

    check point并不直接写入扇区，而是通过封装到box之后写入；如果check point没有分拆，box content就是check point；如果check point分拆到多个box， 将这些有相同uuid的box按照 offset in check point的顺序合并之后才是check point；


## check point 
+ flags

    32bit的开关值，当前可有：

    + 增量： 是否是基于更早的某个check point的增量备份
+ timestamp

    生成该check point的本地时间戳

+ [based on check point]

    当check point是增量备份时存在该字段，写入基于的check point的uuid

+ actions
    
    生成该备份的action序列


## action
+ flags
    32bit的开关值，当前可有：

    + 压缩： action content是否经过压缩
+ relative path

    该action操作的相对路径
+ type

    action的类型
+ content length

    action content的长度（如果经过压缩，是指压缩后的长度）

+ action content

    不同的action type有不同的content格式，有些action是嵌入的结构化数据，有些是非结构化内容；
    比如 action是新增一个文件， 并且压缩， content就是压缩后的新增文件内容；


## 备份和还原过程


### 生成sector
```rust
// max single sector length
const MAX_SECTOR_LENGTH = 32 * 1024 * 1024 * 1024

loop {
    File::open
    for action in checkpoint.actions {

    } 
}

```


### 本地sector meta缓存

## 从target还原sector meta

## 从target读取文件内容



