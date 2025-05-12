use js_sys::{Array, Object, Reflect};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use web_sys::CanvasRenderingContext2d;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// TableConfig 结构体用于保存表格配置
#[wasm_bindgen]
pub struct TableConfig {
    columns: u32,
    rows: u32,
    cell_width: f64,
    cell_height: f64,
    header_height: f64,
    visible_width: f64,
    visible_height: f64,
}

#[wasm_bindgen]
impl TableConfig {
    #[wasm_bindgen(constructor)]
    pub fn new(
        columns: u32,
        rows: u32,
        cell_width: f64,
        cell_height: f64,
        header_height: f64,
        visible_width: f64,
        visible_height: f64,
    ) -> TableConfig {
        TableConfig {
            columns,
            rows,
            cell_width,
            cell_height,
            header_height,
            visible_width,
            visible_height,
        }
    }
}

// 表格单元格内容类型
type CellValue = String;

// 表格行数据
#[derive(Clone)]
struct Row {
    cells: HashMap<String, CellValue>,
}

// 表格数据管理器
#[wasm_bindgen]
pub struct TableManager {
    renderer: TableRenderer,
    data_cache: HashMap<u32, Vec<Row>>,
    segment_size: u32,
}

#[wasm_bindgen]
impl TableManager {
    #[wasm_bindgen(constructor)]
    pub fn new(config: TableConfig, segment_size: u32) -> TableManager {
        log("创建表格管理器");
        TableManager {
            renderer: TableRenderer::new(config),
            data_cache: HashMap::new(),
            segment_size,
        }
    }

    // 获取表格总宽度
    #[wasm_bindgen]
    pub fn get_total_width(&self) -> f64 {
        self.renderer.get_total_width()
    }

    // 获取表格总高度
    #[wasm_bindgen]
    pub fn get_total_height(&self) -> f64 {
        self.renderer.get_total_height()
    }

    // 渲染表头
    #[wasm_bindgen]
    pub fn render_header(&self, ctx: &CanvasRenderingContext2d, scroll_left: f64) {
        // 计算可见区域中的起始/结束列索引
        let start_col = (scroll_left / self.renderer.config.cell_width).floor() as u32;
        let end_col = ((scroll_left + self.renderer.config.visible_width)
            / self.renderer.config.cell_width)
            .floor() as u32;
        let end_col = end_col.min(self.renderer.config.columns - 1);

        self.renderer
            .render_header(ctx, start_col, end_col, scroll_left);
    }

    // 渲染内容区域
    #[wasm_bindgen]
    pub fn render_content(
        &mut self,
        canvas_ctx: &CanvasRenderingContext2d,
        scroll_left: f64,
        scroll_top: f64,
    ) {
        // 计算可见区域的行范围
        let start_row = (scroll_top / self.renderer.config.cell_height).floor() as u32;
        let visible_rows = (self.renderer.config.visible_height / self.renderer.config.cell_height)
            .ceil() as u32
            + 2;
        let end_row = (start_row + visible_rows).min(self.renderer.config.rows - 1);

        // 计算需要加载的数据段
        // let start_segment = start_row / self.segment_size;
        // let end_segment = end_row / self.segment_size;

        // 准备可见数据
        let mut visible_data = Vec::new();
        for row_idx in start_row..=end_row {
            let segment_size = self.segment_size;
            let segment_idx = row_idx / segment_size;
            let segment = self.get_or_load_segment(segment_idx);
            let index_in_segment = (row_idx % segment_size) as usize;

            if index_in_segment < segment.len() {
                visible_data.push(segment[index_in_segment].clone());
            }
        }

        // 转换数据为JS数组
        let js_array = Array::new();
        for row in &visible_data {
            let js_row = Object::new();

            for (key, value) in &row.cells {
                let js_key = JsValue::from_str(key);
                let js_value = JsValue::from_str(value);
                Reflect::set(&js_row, &js_key, &js_value).unwrap();
            }

            js_array.push(&js_row);
        }

        // 调用渲染器的内容渲染方法
        self.renderer.render_content(
            canvas_ctx,
            &js_array.into(),
            scroll_left,
            scroll_top,
            start_row as f64,
        );
    }

    // 加载或获取某个数据段
    fn get_or_load_segment(&mut self, segment_index: u32) -> &Vec<Row> {
        if !self.data_cache.contains_key(&segment_index) {
            let start = segment_index * self.segment_size;
            let end = (start + self.segment_size).min(self.renderer.config.rows);
            let mut segment_data = Vec::with_capacity((end - start) as usize);

            for i in start..end {
                let mut row = Row {
                    cells: HashMap::new(),
                };
                for j in 0..self.renderer.config.columns {
                    row.cells
                        .insert(format!("col_{}", j), format!("数据 {}-{}", i + 1, j + 1));
                }
                segment_data.push(row);
            }

            // 如果缓存过大，清理一些旧数据
            if self.data_cache.len() > 10 {
                self.clean_cache(segment_index);
            }

            self.data_cache.insert(segment_index, segment_data);
        }

        self.data_cache.get(&segment_index).unwrap()
    }

    // 清理缓存数据，保留当前需要的段和临近段
    fn clean_cache(&mut self, current_segment_index: u32) {
        let mut segments_to_remove = Vec::new();
        for &segment_idx in self.data_cache.keys() {
            // 移除距离当前段超过2的段
            if segment_idx < current_segment_index.saturating_sub(2)
                || segment_idx > current_segment_index + 2
            {
                segments_to_remove.push(segment_idx);
            }
        }

        // 只保留最近使用的段
        if segments_to_remove.len() > 5 {
            for segment_idx in segments_to_remove.iter().take(segments_to_remove.len() - 5) {
                self.data_cache.remove(segment_idx);
            }
        }
    }

    // // 预热缓存
    // #[wasm_bindgen]
    // pub fn preload_segments(&mut self, center_segment_index: u32, amount: u32) {
    //     let start = center_segment_index.saturating_sub(amount / 2);
    //     let end = center_segment_index + (amount / 2);

    //     for segment_idx in start..=end {
    //         if segment_idx * self.segment_size < self.renderer.config.rows {
    //             self.get_or_load_segment(segment_idx);
    //         }
    //     }
    // }
    #[wasm_bindgen]
    pub fn configure_hd_canvas(
        &self,
        canvas: &web_sys::HtmlCanvasElement,
        ctx: &CanvasRenderingContext2d,
    ) -> f64 {
        let window = web_sys::window().unwrap();
        let device_pixel_ratio = window.device_pixel_ratio();

        // 获取 canvas 元素的显示大小
        let display_width = canvas.client_width() as u32;
        let display_height = canvas.client_height() as u32;

        // 检查 canvas 的实际大小是否与显示大小匹配
        if canvas.width() != display_width || canvas.height() != display_height {
            // 设置 canvas 的内部大小为显示大小的 DPR 倍
            canvas.set_width((display_width as f64 * device_pixel_ratio) as u32);
            canvas.set_height((display_height as f64 * device_pixel_ratio) as u32);

            // 将所有渲染操作缩放
            ctx.scale(device_pixel_ratio, device_pixel_ratio).unwrap();
        }

        device_pixel_ratio
    }
}

// TableRenderer 结构体处理表格渲染逻辑
#[wasm_bindgen]
pub struct TableRenderer {
    config: TableConfig,
}

#[wasm_bindgen]
impl TableRenderer {
    #[wasm_bindgen(constructor)]
    pub fn new(config: TableConfig) -> TableRenderer {
        TableRenderer { config }
    }

    // 渲染表头
    fn render_header(
        &self,
        ctx: &CanvasRenderingContext2d,
        start_col: u32,
        end_col: u32,
        scroll_left: f64,
    ) {
        // 清空表头画布
        ctx.clear_rect(
            0.0,
            0.0,
            self.config.visible_width,
            self.config.header_height,
        );

        // 填充表头背景
        ctx.set_fill_style_str("#f2f2f2");
        ctx.fill_rect(
            0.0,
            0.0,
            self.config.visible_width,
            self.config.header_height,
        );

        // 表头文本样式
        ctx.set_fill_style_str("#333");
        ctx.set_font("14px Arial");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");

        // 绘制每一列的表头
        for col in start_col..=end_col {
            let x = col as f64 * self.config.cell_width - scroll_left;

            // 绘制文本 - 显示从1开始的列编号
            let text = format!("列 {}", col + 1);
            ctx.fill_text(
                &text,
                x + self.config.cell_width / 2.0,
                self.config.header_height / 2.0,
            )
            .unwrap();
        }

        // 优化批量绘制表头分隔线
        ctx.set_stroke_style_str("#ddd");
        ctx.begin_path();

        for col in start_col..=end_col + 1 {
            let x = col as f64 * self.config.cell_width - scroll_left;
            ctx.move_to(x, 0.0);
            ctx.line_to(x, self.config.header_height);
        }

        // 底部边框
        ctx.move_to(0.0, self.config.header_height);
        ctx.line_to(self.config.visible_width, self.config.header_height);

        ctx.stroke();
    }

    // 计算总宽度
    #[wasm_bindgen]
    pub fn get_total_width(&self) -> f64 {
        self.config.columns as f64 * self.config.cell_width
    }

    // 计算总高度
    #[wasm_bindgen]
    pub fn get_total_height(&self) -> f64 {
        // 分段计算避免精度问题
        let rows_per_segment = 10_000_000;
        let full_segments = self.config.rows / rows_per_segment;
        let remaining_rows = self.config.rows % rows_per_segment;

        (full_segments as f64 * rows_per_segment as f64 * self.config.cell_height)
            + (remaining_rows as f64 * self.config.cell_height)
            + self.config.header_height
    }

    // 添加仅渲染内容的方法
    #[wasm_bindgen]
    pub fn render_content(
        &self,
        canvas_ctx: &CanvasRenderingContext2d,
        data: &JsValue,
        scroll_left: f64,
        scroll_top: f64,
        visible_start_row: f64,
    ) {
        // 清空画布
        canvas_ctx.clear_rect(
            0.0,
            0.0,
            self.config.visible_width,
            self.config.visible_height,
        );

        // 计算可见区域中的起始/结束列索引
        let start_col = (scroll_left / self.config.cell_width).floor() as u32;
        let end_col =
            ((scroll_left + self.config.visible_width) / self.config.cell_width).floor() as u32;
        let end_col = end_col.min(self.config.columns - 1);

        // 计算可见区域中的起始/结束行索引
        let visible_rows = (self.config.visible_height / self.config.cell_height).ceil() + 1.0;
        let start_row = visible_start_row;
        let end_row = (start_row + visible_rows).min(self.config.rows as f64 - 1.0);

        // 绘制表格内容 - 注意这里使用了专门针对内容区域的渲染方法
        self.render_content_cells(
            canvas_ctx,
            data,
            0,                            // 在数据数组中的起始索引是0
            (end_row - start_row) as u32, // 数据数组的结束索引
            start_col,
            end_col,
            scroll_left,
            scroll_top - (start_row * self.config.cell_height), // 调整滚动位置
        );
    }

    // 仅渲染表格单元格内容，不包含表头区域
    fn render_content_cells(
        &self,
        ctx: &CanvasRenderingContext2d,
        data_js: &JsValue,
        data_start_row: u32,
        data_end_row: u32,
        start_col: u32,
        end_col: u32,
        scroll_left: f64,
        adjusted_scroll_top: f64,
    ) {
        // 从 JS 值转换数据
        let data_array = Array::from(data_js);

        // 先批量绘制偶数行背景
        ctx.set_fill_style_str("#ffffff");
        for row_idx in data_start_row..=data_end_row {
            if row_idx >= data_array.length() as u32 {
                break; // 这行很重要，防止访问不存在的数据
            }
            let actual_row = row_idx;
            if actual_row % 2 != 0 {
                continue;
            }

            // 不再加上表头高度，因为表头已经在单独的Canvas中
            let y = (row_idx as f64 * self.config.cell_height) - adjusted_scroll_top;

            // 如果行不可见，跳过
            if y + self.config.cell_height < 0.0 || y > self.config.visible_height {
                continue;
            }

            ctx.fill_rect(0.0, y, self.config.visible_width, self.config.cell_height);
        }

        // 再批量绘制奇数行背景
        ctx.set_fill_style_str("#f9f9f9");
        for row_idx in data_start_row..=data_end_row {
            if row_idx >= data_array.length() as u32 {
                break; // 这行很重要，防止访问不存在的数据
            }
            let actual_row = row_idx;
            if actual_row % 2 == 0 {
                continue;
            }

            // 不再加上表头高度
            let y = (row_idx as f64 * self.config.cell_height) - adjusted_scroll_top;

            // 如果行不可见，跳过
            if y + self.config.cell_height < 0.0 || y > self.config.visible_height {
                continue;
            }

            ctx.fill_rect(0.0, y, self.config.visible_width, self.config.cell_height);
        }

        // 绘制单元格内容
        ctx.set_fill_style_str("#333");
        ctx.set_font("14px Arial");
        ctx.set_text_align("center");
        ctx.set_text_baseline("middle");

        for row_idx in data_start_row..=data_end_row {
            if row_idx >= data_array.length() as u32 {
                break;
            }

            let row_data = data_array.get(row_idx);

            // 不再加上表头高度
            let y = (row_idx as f64 * self.config.cell_height) - adjusted_scroll_top;

            // 如果行不可见，跳过
            if y + self.config.cell_height < 0.0 || y > self.config.visible_height {
                continue;
            }

            for col in start_col..=end_col {
                let x = col as f64 * self.config.cell_width - scroll_left;

                // 如果单元格在水平方向不可见，跳过
                if x + self.config.cell_width < 0.0 || x > self.config.visible_width {
                    continue;
                }

                let key = format!("col_{}", col);
                let key_js = JsValue::from_str(&key);

                // 使用 Reflect.get 从对象获取属性值
                let cell_value = Reflect::get(&row_data, &key_js).unwrap_or(JsValue::undefined());

                let text = if cell_value.is_undefined() {
                    String::from("")
                } else {
                    cell_value.as_string().unwrap_or_default()
                };

                // 绘制单元格文本
                ctx.fill_text(
                    &text,
                    x + self.config.cell_width / 2.0,
                    y + self.config.cell_height / 2.0,
                )
                .unwrap();
            }
        }

        ctx.set_stroke_style_str("#ddd");

        // 获取实际数据长度
        let actual_data_rows = data_array.length() as u32;

        // 对每个实际存在的单元格绘制边框
        for row_idx in data_start_row..=data_end_row {
            // 如果超出实际数据范围，则跳过
            if row_idx >= actual_data_rows {
                break;
            }

            // 计算行的Y坐标
            let y = (row_idx as f64 * self.config.cell_height) - adjusted_scroll_top;

            // 如果行不可见，跳过
            if y + self.config.cell_height < 0.0 || y > self.config.visible_height {
                continue;
            }

            for col in start_col..=end_col {
                let x = col as f64 * self.config.cell_width - scroll_left;

                // 如果单元格在水平方向不可见，跳过
                if x + self.config.cell_width < 0.0 || x > self.config.visible_width {
                    continue;
                }

                // 为每个单元格单独绘制一个矩形边框
                ctx.stroke_rect(x, y, self.config.cell_width, self.config.cell_height);
            }
        }
    }
}

// 初始化函数
#[wasm_bindgen(start)]
pub fn start() {
    // 在这里设置错误处理程序
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    log("WASM 初始化完成");
}
