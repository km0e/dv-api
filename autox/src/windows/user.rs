pub struct AutoX {}

impl AutoX {
    pub fn new() -> Result<Self, auto_launch::Error> {
        Ok(Self {})
    }
    pub fn setup(
        &self,
        name: impl AsRef<str>,
        cmd: impl AsRef<str>,
    ) -> Result<(), auto_launch::Error> {
        let name = name.as_ref();
        let cmd = cmd.as_ref();
        let cmds = cmd.split_whitespace().collect::<Vec<_>>();
        auto_launch::AutoLaunch::new(name, cmds[0], &cmds[1..]).enable()
    }
    pub fn destroy(&self, name: impl AsRef<str>) -> Result<(), auto_launch::Error> {
        auto_launch::AutoLaunch::new(name.as_ref(), "", &[""]).disable()
    }
    pub fn reload(&self, _name: impl AsRef<str>) -> Result<(), auto_launch::Error> {
        Ok(())
    }
}
