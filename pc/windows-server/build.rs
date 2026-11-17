// 嵌入应用图标（PE 资源）：资源管理器 / 任务栏 / 控制面板卸载项显示桌链双环相链图标
fn main() {
    embed_resource::compile("assets\\vrc-server.rc", embed_resource::NONE);
}
