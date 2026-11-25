//! Дымовой тест CLI: собранный бинарник `vtt demo` проходит сценарий целиком.

use std::process::Command;

#[test]
fn demo_command_succeeds() {
    let exe = env!("CARGO_BIN_EXE_vtt");
    let output = Command::new(exe)
        .arg("demo")
        .output()
        .expect("бинарник должен запускаться");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "demo завершилось с ошибкой.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Демо завершено успешно"),
        "нет финальной строки демо:\n{stdout}"
    );
}
