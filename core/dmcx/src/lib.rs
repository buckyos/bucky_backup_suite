pub use chunk;

// 这里可以添加 dmcx 特定的功能
pub fn dmcx_function() {
    println!("This is a function from dmcx crate");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        dmcx_function();
        // 这里可以添加更多测试
    }
}
