const { getCurrentWebview } = window.__TAURI__.webview
const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event

const overlay = document.getElementById('drop-overlay')

await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === 'over') {
        overlay.classList.add('active')
    } else if (e.payload.type === 'drop') {
        overlay.classList.remove('active')
        handleDrop(e.payload.paths)
    } else {
        overlay.classList.remove('active')
    }
})

function handleDrop(file_paths) {
    invoke('process_dropped_files', { paths: file_paths })
}

const outputElem = document.getElementById('output')
listen('ldb_csv', e => {
    if (outputElem) {
        outputElem.textContent = e.payload
    }
})