use super::*;
use crate::automap;
use std::ops::RangeInclusive;

pub(crate) const MODULE_ENABLED: usize = 0x14;

pub(crate) struct ModuleSpec<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub category: usize,
    pub gui: unsafe extern "system" fn(*mut c_void, *mut c_void),
    pub sync: unsafe extern "system" fn(*mut c_void),
    pub callbacks: &'a [(usize, usize, bool)],
}

pub(crate) struct Catalog {
    template: usize,
    manager: usize,
    layout: automap::module_manager::Layout,
    master: Vec<usize>,
}

impl Catalog {
    pub fn open(base: usize, supported: RangeInclusive<usize>) -> Result<Self> {
        automap::require(&["module_manager", "block_in_module"])?;
        let manager = base + automap::rva("module_manager");
        let layout = automap::module_manager::layout()?;
        let master_at = manager + layout.master;
        let count = read_usize(master_at + 8, "module count")?;
        if !supported.contains(&count) {
            return Err(Error::Jvm(format!(
                "unsupported Slinky module count {count} (expected {supported:?})"
            )));
        }
        let master = read_pointer_array(read_usize(master_at, "module array")?, count, "modules")?;
        Ok(Self {
            template: base + automap::rva("block_in_module"),
            manager,
            layout,
            master,
        })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.master
            .iter()
            .any(|&entry| module_title(entry).as_deref() == Some(name))
    }

    pub fn prepare(mut self, spec: ModuleSpec<'_>) -> Result<PreparedModule> {
        if spec.category >= 5 {
            return Err(Error::Jvm("invalid custom module category".into()));
        }
        let mut descriptor = clone_aligned(self.template, 0x210, spec.name)?;
        set_usize(&mut descriptor, 0, leak_cstring(spec.name)?);
        set_usize(&mut descriptor, 8, leak_cstring(spec.description)?);
        set_u32(&mut descriptor, 0x10, spec.category as u32);
        set_u8(&mut descriptor, MODULE_ENABLED, 0);
        set_u32(&mut descriptor, 0x18, 0);
        set_u32(&mut descriptor, 0x1c, 0);
        zero_range(&mut descriptor, 0x48, 0x1f8);
        set_usize(&mut descriptor, 0x1f8, spec.gui as *const () as usize);
        set_usize(&mut descriptor, 0x200, spec.sync as *const () as usize);
        let descriptor = leak_words(descriptor);
        self.master.push(descriptor);
        sort_modules(&mut self.master);
        let count = self.master.len();
        let master_at = self.manager + self.layout.master;
        let category_at = self.manager + self.layout.categories + spec.category * 0x10;
        let mut category = read_pointer_array(
            read_usize(category_at, "category array")?,
            read_usize(category_at + 8, "category count")?,
            "category",
        )?;
        category.push(descriptor);
        sort_modules(&mut category);
        let category_count = category.len();
        let mut writes = vec![
            (master_at, leak_pointer_array(self.master)),
            (category_at, leak_pointer_array(category)),
        ];
        for ptr_at in self.layout.pending {
            let mut pending = vec![0; count];
            copy_pointer_prefix(
                read_usize(self.manager + ptr_at, "pending pointer")?,
                read_usize(self.manager + ptr_at + 8, "pending count")?,
                &mut pending,
                "pending",
            )?;
            writes.push((self.manager + ptr_at, leak_pointer_array(pending)));
        }
        writes.extend(prepare_listeners(self.manager, spec.callbacks)?);
        writes.push((category_at + 8, category_count));
        writes.push((master_at + 8, count));
        Ok(PreparedModule { descriptor, writes })
    }
}

pub(crate) struct PreparedModule {
    pub descriptor: usize,
    writes: Vec<(usize, usize)>,
}

impl PreparedModule {
    pub fn publish(self) -> Result<()> {
        let _threads = SuspendedThreads::suspend_others()?;
        for &(at, value) in &self.writes {
            write_ptr(at, value, "custom module publication")?;
        }
        drop(_threads);
        Ok(())
    }
}

