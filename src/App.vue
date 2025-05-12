<script setup lang="ts">
import { nextTick, onMounted, ref, watch, computed, onBeforeUnmount } from 'vue';
import init, { TableConfig, TableManager } from '../canvas-wasm/pkg';


// 基础配置
const config = {
  columns: 50,
  rows: 1000000000,
  cellWidth: 120,
  cellHeight: 50,
  headerHeight: 40,
  visibleWidth: document.documentElement.clientWidth,
  visibleHeight: document.documentElement.clientHeight - 40,
  segmentSize: 1000, // 每段1000行

  // 添加安全上限
  maxSafeRowIndex: 999999999,
};

// 状态变量，添加适当的类型
const wasmLoaded = ref<boolean>(false);
let tableConfig: TableConfig;
let tableManager: TableManager;// 使用 any 或创建合适的类型
const canvasRef = ref<HTMLCanvasElement | null>(null);
const canvasHeaderRef = ref<HTMLCanvasElement | null>(null);
const scrollLeft = ref<number>(0);
const scrollTop = ref<number>(0);
const containerRef = ref<HTMLDivElement | null>(null);
const totalWidth = ref<number>(0);
const totalHeight = ref<number>(0);

// 水平滚动相关状态
const horizontalIsDragging = ref(false);
const horizontalScrollbarWidth = ref(100);
let horizontalInitialClientX = 0;
let horizontalInitialScrollLeft = 0;
// 计算水平滚动相关值
const horizontalMaxScroll = computed(() => {
  // 确保避免除零问题
  if (totalWidth.value <= config.visibleWidth) return 0;
  return totalWidth.value - config.visibleWidth;
});

// 计算水平滚动比例 - 确保安全计算
const horizontalScrollRatio = computed(() => {
  // 加入安全检查，避免除以零或极小的值
  if (horizontalMaxScroll.value <= 0) return 0;

  // 使用有限的精度
  const ratio = scrollLeft.value / horizontalMaxScroll.value;
  return Math.max(0, Math.min(1, ratio)); // 确保在 0-1 范围内
});

// 添加表头渲染相关变量
const headerCtx = ref<CanvasRenderingContext2D | null>(null);
const isHeaderInitialized = ref<boolean>(false);

// 计算水平滚动条样式
const horizontalScrollbarStyle = computed(() => {
  // 容器宽度
  const containerWidth = config.visibleWidth - 12; // 减去垂直滚动条宽度

  // 计算滚动条宽度
  const width = Math.max(20, (containerWidth / totalWidth.value) * containerWidth);
  horizontalScrollbarWidth.value = width;

  // 计算滚动条位置
  const left = horizontalScrollRatio.value * (containerWidth - width);

  return {
    width: `${width}px`,
    left: `${left}px`,
    display: totalWidth.value > config.visibleWidth ? 'block' : 'none'
  };
});

// 处理水平滚动条点击事件
const handleHorizontalScrollbarClick = (e: MouseEvent) => {
  if ((e?.target as HTMLElement).classList.contains('scrollbar-thumb')) {
    return;
  }

  // 获取容器相对于视口的位置
  const containerRect = containerRef.value!.getBoundingClientRect();
  // 计算点击位置相对于容器的偏移
  const offsetX = e.clientX - containerRect.left;
  // 计算点击位置在容器中的比例
  const ratio = offsetX / (containerRef.value!.clientWidth - 12); // 减去垂直滚动条宽度
  // 根据比例计算滚动位置
  const newScrollLeft = Math.max(0, Math.min(horizontalMaxScroll.value, ratio * horizontalMaxScroll.value));

  scrollLeft.value = newScrollLeft;
  renderHeader();
  renderTable();
};

// 处理水平滚动条拖拽开始
const handleHorizontalScrollbarDragStart = (e: MouseEvent) => {
  e.preventDefault();
  horizontalIsDragging.value = true;

  // 记录初始点击位置和滚动位置
  horizontalInitialClientX = e.clientX;
  horizontalInitialScrollLeft = scrollLeft.value;

  // 添加临时事件监听器
  document.addEventListener('mousemove', handleHorizontalDragMove);
  document.addEventListener('mouseup', handleHorizontalDragEnd);
};

// 水平拖拽移动处理函数
const handleHorizontalDragMove = (moveEvent: MouseEvent) => {
  if (!horizontalIsDragging.value) return;

  // 计算鼠标移动距离
  const deltaX = moveEvent.clientX - horizontalInitialClientX;
  // 计算容器宽度
  const containerWidth = containerRef.value!.clientWidth - 12; // 减去垂直滚动条宽度
  // 根据移动距离计算滚动位置变化
  const scrollDelta = (deltaX / containerWidth) * horizontalMaxScroll.value;
  // 应用新的滚动位置
  const newScrollLeft = Math.max(0, Math.min(horizontalMaxScroll.value, horizontalInitialScrollLeft + scrollDelta));

  scrollLeft.value = newScrollLeft;
  renderHeader();
  renderTable();
};

// 水平拖拽结束处理函数
const handleHorizontalDragEnd = () => {
  horizontalIsDragging.value = false;
  // 移除事件监听器
  document.removeEventListener('mousemove', handleHorizontalDragMove);
  document.removeEventListener('mouseup', handleHorizontalDragEnd);
};


// 真实尺寸计算 - 不受DOM限制影响
const realTotalHeight = computed(() => calculateTotalHeight(config.rows, config.cellHeight));
const realMaxScroll = computed(() => realTotalHeight.value - config.visibleHeight);
const realScrollRatio = computed(() => {
  // 加入安全检查，避免除以零或极小的值
  if (realMaxScroll.value <= 0) return 0;

  // 使用有限的精度
  const ratio = scrollTop.value / realMaxScroll.value;
  return Math.max(0, Math.min(1, ratio)); // 确保在 0-1 范围内
});
const getCurrentRow = computed(() => Math.floor(scrollTop.value / config.cellHeight));


// 添加滚动条交互状态
const isDragging = ref(false);
const scrollbarHeight = ref(50);
let initialClientY = 0;
let initialScrollTop = 0;



// 处理滚动条点击事件
const handleScrollbarClick = (e: MouseEvent) => {
  if ((e?.target as HTMLElement).classList.contains('scrollbar-thumb')) {
    // 如果点击的是滑块本身，不处理(避免与拖拽冲突)
    return;
  }

  // 获取容器相对于视口的位置
  const containerRect = containerRef.value?.getBoundingClientRect();
  if (containerRect === undefined) return;
  // 计算点击位置相对于容器的偏移
  const offsetY = e.clientY - containerRect.top;
  // 计算点击位置在容器中的比例
  const ratio = offsetY / containerRef.value!.clientHeight;
  // 根据比例计算滚动位置 - 使用真实滚动空间
  const newScrollTop = Math.max(0, Math.min(realMaxScroll.value, ratio * realMaxScroll.value));

  const maxScrollTop = totalHeight.value - config.visibleHeight;
  scrollTop.value = Math.min(newScrollTop, maxScrollTop);
  renderTable();
};

// 处理滚动条拖拽开始
const handleScrollbarDragStart = (e: MouseEvent) => {
  e.preventDefault();
  isDragging.value = true;

  // 记录初始点击位置和滚动位置
  initialClientY = e.clientY;
  initialScrollTop = scrollTop.value;

  // 添加临时事件监听器
  document.addEventListener('mousemove', handleDragMove);
  document.addEventListener('mouseup', handleDragEnd);
};

// 拖拽移动处理函数
const handleDragMove = (moveEvent: MouseEvent) => {
  if (!isDragging.value) return;

  // 计算鼠标移动距离
  const deltaY = moveEvent.clientY - initialClientY;
  // 计算容器高度
  const containerHeight = containerRef.value!.clientHeight;
  // 根据移动距离计算滚动位置变化 - 使用真实滚动空间
  const scrollDelta = (deltaY / containerHeight) * realMaxScroll.value;
  // 应用新的滚动位置
  const newScrollTop = Math.max(0, Math.min(realMaxScroll.value, initialScrollTop + scrollDelta));

  scrollTop.value = newScrollTop;
  renderTable();
};

// 拖拽结束处理函数
const handleDragEnd = () => {
  isDragging.value = false;
  // 移除事件监听器
  document.removeEventListener('mousemove', handleDragMove);
  document.removeEventListener('mouseup', handleDragEnd);
};
// 计算真实总高度的函数，但设计为避免直接大数值乘法
const calculateTotalHeight = (rows: number, cellHeight: number) => {
  const MAX_SAFE_ROWS_PER_CHUNK = 10000000; // 1千万行一组安全计算
  const fullChunks = Math.floor(rows / MAX_SAFE_ROWS_PER_CHUNK);
  const remainingRows = rows % MAX_SAFE_ROWS_PER_CHUNK;

  // 分块计算，避免单次乘法过大
  return (fullChunks * MAX_SAFE_ROWS_PER_CHUNK * cellHeight) +
    (remainingRows * cellHeight) +
    config.headerHeight;
};


// 计算滚动条位置和大小
const scrollbarStyle = computed(() => {
  // 容器高度
  const containerHeight = config.visibleHeight;

  // 计算滚动条高度 - 使用真实比例
  const height = Math.max(20, (containerHeight / realTotalHeight.value) * containerHeight);
  scrollbarHeight.value = height;

  // 计算滚动条位置 - 使用真实比例
  const top = realScrollRatio.value * (containerHeight - height);
  return {
    height: `${height}px`,
    top: `${top}px`,
    display: 'block'
  };
});

// 表格跳转到指定行
const scrollToRow = (rowIndex: number) => {
  // 确保不超过安全上限
  const safeRowIndex = Math.min(rowIndex, config.rows - 1);

  // 使用分段计算以避免大数值精度问题
  // 每次计算最多处理1亿行的高度
  const MAX_ROWS_PER_CALCULATION = 100000000;

  let rowPosition = 0;
  const fullChunks = Math.floor(safeRowIndex / MAX_ROWS_PER_CALCULATION);
  const remainingRows = safeRowIndex % MAX_ROWS_PER_CALCULATION;

  rowPosition = (fullChunks * MAX_ROWS_PER_CALCULATION * config.cellHeight) +
    (remainingRows * config.cellHeight);

  scrollTop.value = Math.min(rowPosition, realMaxScroll.value);
  renderTable();
};

// 初始化 WASM 和表格管理器
const initializeWasm = async () => {
  try {
    console.log("开始加载 WASM 模块...");
    await init();
    console.log("WASM 加载成功");
    tableConfig = new TableConfig(
      config.columns,
      config.rows,
      config.cellWidth,
      config.cellHeight,
      config.headerHeight,
      config.visibleWidth,
      config.visibleHeight
    )

    tableManager = new TableManager(tableConfig, config.segmentSize)


    totalWidth.value = tableManager.get_total_width();
    totalHeight.value = realTotalHeight.value;

    // 预加载前几段数据
    // tableManager.preload_segments(0, 3);

    wasmLoaded.value = true;

    await nextTick();

    // 初始化表头
    renderHeader();

    // 初始化表格内容
    if (canvasRef.value) {
      renderTable();
    } else {
      console.error("Canvas 元素未找到");
    }
  } catch (error) {
    console.error('Failed to load WASM module:', error);
  }
};
const renderHeader = () => {
  if (!wasmLoaded.value || !tableManager || !canvasHeaderRef.value) {
    console.warn("表头渲染条件不满足");
    return;
  }

  if (!headerCtx.value) {
    headerCtx.value = canvasHeaderRef.value.getContext('2d');
    if (!headerCtx.value) {

      return;
    }
  }

  try {

    tableManager.render_header(headerCtx.value, scrollLeft.value);
    isHeaderInitialized.value = true;
  } catch (error) {
    console.error('表头渲染错误:', error);
  }
};
// 渲染表格
// 渲染表格前进行检查
const renderTable = () => {
  if (!wasmLoaded.value || !tableManager || !canvasRef.value) {

    return;
  }

  // 检查数值是否合理
  if (!isFinite(scrollTop.value) || scrollTop.value < 0) {

    scrollTop.value = 0;
  }

  const currentRow = getCurrentRow.value;
  if (currentRow > config.rows) {

    scrollTop.value = 0;
  }

  const ctx = canvasRef.value.getContext('2d');
  if (!ctx) {

    return;
  }

  try {
    // 当水平滚动改变时更新表头
    if (!isHeaderInitialized.value) {
      renderHeader();
    }
    tableManager.configure_hd_canvas(canvasRef.value!, ctx);
    tableManager.render_content(
      ctx,
      scrollLeft.value,
      scrollTop.value
    );
    // 配置高 DPI 支持


  } catch (error) {

    console.error('渲染错误:', error);
  }
};


// 处理滚动事件
const handleWheel = (e: WheelEvent) => {
  // 按住Shift键进行水平滚动
  if (e.shiftKey) {
    e.preventDefault();
    const delta = e.deltaX || e.deltaY; // 如果deltaX为0，则使用deltaY
    const scrollSpeed = e.deltaMode === 1 ? 40 : 1;
    const newScrollLeft = Math.max(0, Math.min(
      horizontalMaxScroll.value,
      scrollLeft.value + (delta * scrollSpeed)
    ));

    if (scrollLeft.value !== newScrollLeft) {
      scrollLeft.value = newScrollLeft;
      renderHeader();
      renderTable();
    }
    return;
  }

  // 垂直滚动逻辑保持不变
  e.preventDefault();
  const scrollSpeed = e.deltaMode === 1 ? 40 : 1;
  const delta = e.deltaY * scrollSpeed;
  const isApproachingMax = scrollTop.value > realMaxScroll.value * 0.95;
  const adjustedDelta = isApproachingMax ? delta / 10 : delta;
  const newScrollTop = Math.max(0, Math.min(realMaxScroll.value, scrollTop.value + adjustedDelta));

  if (scrollTop.value !== newScrollTop) {
    scrollTop.value = newScrollTop;
    requestAnimationFrame(renderTable);
  }
};
watch(() => scrollLeft, () => {
  renderHeader(); // 重新渲染表头
});
// 组件挂载
onMounted(async () => {
  console.log("组件已挂载");
  await initializeWasm();

  if (containerRef.value) {
    // 使用wheel事件替代scroll事件
    containerRef.value.addEventListener('wheel', handleWheel, { passive: false });
  }
});
onBeforeUnmount(() => {
  if (containerRef.value) {
    containerRef.value.removeEventListener('wheel', handleWheel);
  }

  // 清理WASM资源
  if (tableManager) {
    tableManager.free();
  }
  if (tableConfig) {
    tableConfig.free();
  }

});
</script>

<template>
  <div>
    <div v-if="wasmLoaded">
      <div class="table-wrapper" :style="{
        width: `${config.visibleWidth}px`,
        height: `${config.visibleHeight + config.headerHeight}px`
      }">
        <!-- 表头容器，固定在顶部 -->
        <div class="header-container" :style="{
          width: `${config.visibleWidth}px`,
          height: `${config.headerHeight}px`
        }">
          <canvas ref="canvasHeaderRef" :width="config.visibleWidth" :height="config.headerHeight"></canvas>
        </div>

        <!-- 内容容器 -->
        <div ref="containerRef" class="content-container" :style="{
          width: `${config.visibleWidth}px`,
          height: `${config.visibleHeight}px`
        }">
          <canvas ref="canvasRef" class="content-canvas" :width="config.visibleWidth"
            :height="config.visibleHeight"></canvas>

          <!-- 垂直滚动条 -->
          <div class="scrollbar-track vertical" @click="handleScrollbarClick">
            <div class="scrollbar-thumb" :style="scrollbarStyle" @mousedown="handleScrollbarDragStart"></div>
          </div>

          <!-- 水平滚动条 -->
          <div class="scrollbar-track horizontal" @click="handleHorizontalScrollbarClick">
            <div class="scrollbar-thumb horizontal" :style="horizontalScrollbarStyle"
              @mousedown="handleHorizontalScrollbarDragStart"></div>
          </div>
        </div>
      </div>

      <!-- 调试面板 -->
      <div class="debug-panel">
        <div>总行数: {{ config.rows.toLocaleString() }}</div>
        <button @click="scrollToRow(0)">首行</button>
        <button @click="scrollToRow(1000000)">100万行</button>
        <button @click="scrollToRow(100000000)">1亿行</button>
        <button @click="scrollToRow(999999999)">末行</button>
      </div>
    </div>
    <div v-else class="loading">
      加载 WebAssembly 模块中...
    </div>
  </div>
</template>

<style scoped>
.table-wrapper {
  overflow: hidden;
}

.header-container {
  position: fixed;
  top: 0;
  left: 0;
  z-index: 20;
  background: #fff;
  border-bottom: 1px solid #ddd;
  overflow: hidden;
}

.content-container {
  overflow: hidden;
  margin-top: 40px;
  position: relative;
  -ms-overflow-style: none;
  /* IE and Edge */
  scrollbar-width: none;
  /* Firefox */
}

.content-canvas {
  position: fixed;
  top: 40px;
  left: 0;
  z-index: 10;
  pointer-events: none;
}



.content-container::-webkit-scrollbar {
  display: none;
  /* Chrome, Safari and Opera */
}

/* 垂直滚动条轨道 */
.scrollbar-track.vertical {
  position: absolute;
  right: 0;

  width: 12px;
  height: calc(100% - 12px);
  /* 留出底部水平滚动条的空间 */
  background: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  cursor: pointer;
  z-index: 30;
}

/* 水平滚动条轨道 */
.scrollbar-track.horizontal {
  position: fixed;
  bottom: 0;
  left: 0;
  width: calc(100% - 12px);
  /* 留出右侧垂直滚动条的空间 */
  height: 12px;
  background: rgba(0, 0, 0, 0.05);
  border-radius: 6px;
  cursor: pointer;
  z-index: 30;
}

/* 通用滚动条滑块样式 */
.scrollbar-thumb {
  position: absolute;
  background: rgba(0, 0, 0, 0.3);
  border-radius: 4px;
  cursor: pointer;
  transition: background-color 0.2s;
}

/* 垂直滚动条滑块 */
.scrollbar-track.vertical .scrollbar-thumb {
  width: 8px;
  right: 2px;
}

/* 水平滚动条滑块 */
.scrollbar-track.horizontal .scrollbar-thumb {
  height: 8px;
  bottom: 2px;
}

.scrollbar-thumb:hover,
.scrollbar-thumb:active {
  background: rgba(0, 0, 0, 0.5);
}

/* 其他样式保持不变 */
.debug-panel {
  position: absolute;
  bottom: 10px;
  right: 10px;
  background: rgba(255, 255, 255, 0.9);
  padding: 10px;
  border: 1px solid #ddd;
  z-index: 1000;
}

.debug-panel button {
  margin: 5px;
  padding: 3px 10px;
}

.loading {
  padding: 20px;
  font-size: 18px;
  color: #666;
}
</style>