const { getCurrentWebview } = window.__TAURI__.webview
const { invoke } = window.__TAURI__.core
const { listen } = window.__TAURI__.event
const { writeText, readText } = window.__TAURI__.clipboardManager

const overlay = document.getElementById('drop-overlay')
const dropAreaWrapper = document.getElementById('drop-area-wrapper')
const contentWrapper = document.getElementById('wrapper')

const recordsButton = document.getElementById('records-button')
const manifestButton = document.getElementById('manifest-button')
const logButton = document.getElementById('log-button')

const loadingIndicator = document.getElementById('loading-indicator')

// helper ----------------------------------------------------------------------
function parseCsvLine(line) {
    const result = []
    let current = ''
    let inQuotes = false

    for (let i = 0; i < line.length; i++) {
        const char = line[i]
        if (char === '"') {
            if (inQuotes && line[i + 1] === '"') {
                current += '"'
                i++ // skip next quote
            } else {
                inQuotes = !inQuotes
            }
        } else if (char === ',' && !inQuotes) {
            result.push(current)
            current = ''
        } else {
            current += char
        }
    }
    result.push(current)
    return result
}

await getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === 'over') {
        overlay.classList.add('active')
    } else if (e.payload.type === 'drop') {
        overlay.classList.remove('active')
        dropAreaWrapper.style.display = 'none'
        contentWrapper.style.display = 'block'
        invoke('process_dropped_files', { paths: e.payload.paths })
    } else {
        overlay.classList.remove('active')
    }
})

const CUT_OFF_LEN = 300
const largeValRenderer = (params) => {
    const fullValue = params.value ?? ''
    const isLarge = fullValue.length > CUT_OFF_LEN

    const container = document.createElement('span')
    container.textContent = isLarge ? fullValue.substring(0, CUT_OFF_LEN) : fullValue

    if (isLarge) {
        const remainingChars = fullValue.length - CUT_OFF_LEN
        const badge = document.createElement('span')
        badge.style.color = 'red'
        badge.textContent = ` [+${remainingChars.toLocaleString()} Chars]`
        container.appendChild(badge)
    }

    return container
}

const valuePopup = document.getElementById('value-popup')
const popupContent = document.getElementById('popup-content')

function escapeHtml(text) {
    return text.replace(/[&<>"']/g, function (m) {
        switch (m) {
            case '&': return '&amp;'
            case '<': return '&lt;'
            case '>': return '&gt;'
            case '"': return '&quot;'
            case "'": return '&#39;'
            default: return m
        }
    })
}

function showValuePopup(value) {
    const activeTabButton = document.querySelector('.active-tab-button')

    let searchTerm = ''
    if (activeTabButton) {
        const tabId = activeTabButton.id.replace('-button', '')
        const searchInput = document.getElementById(`${tabId}-search-input`)
        if (searchInput && searchInput.value.trim()) {
            searchTerm = searchInput.value.trim()
        }
    }

    let filterTerm = ''
    if (activeTabButton && activeTabButton.id === 'records-button') {
        const filterModel = recordsGrid.getFilterModel()
        if (filterModel.V && filterModel.V.filter) {
            filterTerm = filterModel.V.filter.trim()
        }
    }

    let allTerms = []
    if (searchTerm) allTerms.push(searchTerm)
    if (filterTerm) allTerms.push(filterTerm)
    allTerms = allTerms.join(' ').trim()

    if (!allTerms) {
        popupContent.textContent = value
    } else {
        // escape HTML and highlight search phrase
        let html = escapeHtml(value)

        const phrase = allTerms.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
        if (phrase) {
            const regex = new RegExp(phrase, 'gi')
            html = html.replace(regex, match => `<mark>${match}</mark>`)
        }

        popupContent.innerHTML = html
    }
    valuePopup.style.display = 'flex'
}

// records-grid ----------------------------------------------------------------
const gridOptionsRecords = {
    columnDefs: [
        {
            field: "Seq",
            comparator: (valueA, valueB) => valueA - valueB,
            headerName: "Seq. #",
            flex: 0.4,
            minWidth: 40,
        },
        { field: "K", headerName: "Key", flex: 2, minWidth: 100 },
        {
            field: "V", headerName: "Value", flex: 5, minWidth: 100,
            cellRenderer: largeValRenderer,
            onCellDoubleClicked: (params) => {
                if (params.value) {
                    showValuePopup(params.value)
                }
            },
        },
        { field: "Cr", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "St", headerName: "State", flex: 0.4, minWidth: 40 },
        { field: "BO", comparator: (valueA, valueB) => valueA - valueB, headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "C", headerName: "Compressed", flex: 0.4, minWidth: 90, cellStyle: { pointerEvents: 'none' } },
        { field: "F", headerName: "File", flex: 0.4, minWidth: 80 },
        { field: "FP", headerName: "File Path", flex: 0.4, minWidth: 80 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
    getRowStyle: params => {
        if (params.data && params.data.Cr && params.data.Cr.includes('failed')) {
            return { color: 'red' }
        } else if (params.data && params.data.St && params.data.St.includes('deleted')) {
            return { backgroundColor: '#f2f2f6' }
        }
        return null
    }
}

const recordsGridElem = document.querySelector('#records-grid')
const recordsGrid = agGrid.createGrid(recordsGridElem, gridOptionsRecords)

// manifest-grid ---------------------------------------------------------------
const gridOptionsManifest = {
    columnDefs: [
        { field: "Tag", flex: 0.5, minWidth: 100 },
        { field: "TagValue", headerName: "Value", flex: 5, minWidth: 200 },
        { field: "CRC", headerName: "CRC32", flex: 0.4, minWidth: 40 },
        { field: "BlockOffset", headerName: "Block Offset", flex: 0.4, minWidth: 90 },
        { field: "File", flex: 0.4, minWidth: 115 },
        { field: "FilePath", headerName: "File Path", flex: 0.4, minWidth: 115 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
    getRowStyle: params => {
        if (params.data && params.data.CRC && params.data.CRC.includes('failed')) {
            return { color: 'red' }
        }
        return null
    }
}

const manifestGridElem = document.querySelector('#manifest-grid')
const manifestGrid = agGrid.createGrid(manifestGridElem, gridOptionsManifest)

// log-text-grid ---------------------------------------------------------------
const gridOptionsLogText = {
    columnDefs: [
        { field: "Date", flex: 0.3, minWidth: 150, sort: 'asc' },
        { field: "ThreadId", headerName: "ThreadID", flex: 0.2, minWidth: 80 },
        { field: "Msg", headerName: "Message", flex: 5, minWidth: 300 },
        { field: "File", flex: 0.2, minWidth: 80 },
        { field: "FilePath", headerName: "File Path", flex: 0.2, minWidth: 80 },
    ],
    defaultColDef: {
        filter: true,
    },
    rowData: [],
    overlayLoadingTemplate: '<p style="font-weight: bold; color: orangered;">Loading...</p>',
    overlayNoRowsTemplate: '<p style="font-weight: bold; color: orangered;">No Data</p>',
    animateRows: false,
    rowBuffer: 50,
    debounceVerticalScrollbar: true,
}

const logTextGridElem = document.querySelector('#log-text-grid')
const logTextGrid = agGrid.createGrid(logTextGridElem, gridOptionsLogText)

// listener --------------------------------------------------------------------
function onFilterChanged() {
    updateRowCount()
    updateFilterResetButtonState()
}

function updateFilterResetButtonState() {
    const activeTab = document.querySelector('.active-tab-button').id
    let gridApi, searchInput

    if (activeTab === 'records-button') {
        gridApi = recordsGrid
        searchInput = document.getElementById('records-search-input')
    } else if (activeTab === 'manifest-button') {
        gridApi = manifestGrid
        searchInput = document.getElementById('manifest-search-input')
    } else if (activeTab === 'log-button') {
        gridApi = logTextGrid
        searchInput = document.getElementById('log-search-input')
    }

    const hasFilter = gridApi && gridApi.getFilterModel() && Object.keys(gridApi.getFilterModel()).length > 0
    const hasSearch = searchInput && searchInput.value.trim().length > 0

    document.getElementById('filter-reset-button').disabled = !(hasFilter || hasSearch)
}

recordsGrid.addEventListener('filterChanged', onFilterChanged)
manifestGrid.addEventListener('filterChanged', onFilterChanged)
logTextGrid.addEventListener('filterChanged', onFilterChanged)

let processingTime = null
listen('processing_started', () => {
    loadingIndicator.style.display = 'block'
    processingTime = performance.now()
})

listen('processing_finished', () => {
    loadingIndicator.style.display = 'none'
    if (processingTime) {
        const duration = ((performance.now() - processingTime) / 1000).toFixed(2)
        const procTime = document.getElementById('processing-time')
        procTime.firstElementChild.textContent = duration
        procTime.style.display = 'block'
        processingTime = null
    }
})

let isFirstLoad = true
listen('records_csv', e => {
    const csv = e.payload
    // console.log(csv)
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            if (header === "C") {
                obj[header] = values[idx] === "true"
            } else {
                obj[header] = values[idx]
            }
        })
        return obj
    })

    recordsGrid.applyTransaction({ add: rowData })

    if (isFirstLoad) {
        showTab('records')
        isFirstLoad = false
    }
    updateRowCount()
})

listen('manifest_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            obj[header] = values[idx]
        })
        return obj
    })

    manifestGrid.applyTransaction({ add: rowData })

    if (!recordsButton.classList.contains('active-tab-button')) {
        showTab('manifest')
    }
    updateRowCount()
})

listen('log_text_csv', e => {
    const csv = e.payload
    const [headerLine, ...lines] = csv.trim().split('\n')
    const headers = parseCsvLine(headerLine)

    const rowData = lines.map(line => {
        const values = parseCsvLine(line)
        const obj = {}
        headers.forEach((header, idx) => {
            obj[header] = values[idx]
        })
        return obj
    })

    logTextGrid.applyTransaction({ add: rowData })

    if (!recordsButton.classList.contains('active-tab-button') && !manifestButton.classList.contains('active-tab-button')) {
        showTab('log')
    }
    updateRowCount()
})

// value popup
document.addEventListener('mousedown', (e) => {
    if (valuePopup.style.display !== 'none' && e.target.id !== 'popup-content') {
        valuePopup.style.display = 'none'
    }
})

document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && valuePopup.style.display !== 'none') {
        valuePopup.style.display = 'none'
    }
})

// copy to clipboard
document.addEventListener('keydown', async (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'c' && valuePopup.style.display === 'none') {
        e.preventDefault()

        const activeTab = document.querySelector('.active-tab-button').id
        let activeGrid

        if (activeTab === 'records-button') {
            activeGrid = recordsGrid
        } else if (activeTab === 'manifest-button') {
            activeGrid = manifestGrid
        } else if (activeTab === 'log-button') {
            activeGrid = logTextGrid
        }

        if (activeGrid) {
            const focusedCell = getFocusedCellValue(activeGrid)
            if (focusedCell !== undefined) {
                await writeText(focusedCell)
            }
        }
    }
})

function getFocusedCellValue(gridApi) {
    const focusedCell = gridApi.getFocusedCell()
    const rowNode = gridApi.getDisplayedRowAtIndex(focusedCell.rowIndex)
    return rowNode.data[focusedCell.column.getColId()]
}

// search inputs (quick filter)
let searchTimeout = null
let searchFrame = null
document.querySelectorAll('[id$="search-input"]').forEach(inputElement => {
    const prefix = inputElement.id.replace('-search-input', '')
    const inputContainer = inputElement.closest('.input-container')
    inputContainer.style.display = 'none'

    const clearButton = document.getElementById(`${prefix}-clear-button`)
    clearButton.style.display = 'none'

    let gridApi
    switch (prefix) {
        case 'records':
            gridApi = recordsGrid
            break
        case 'manifest':
            gridApi = manifestGrid
            break
        case 'log':
            gridApi = logTextGrid
            break
    }

    inputElement.addEventListener('input', function () {
        gridApi.setGridOption('loading', true)

        if (clearButton) {
            clearButton.style.display = this.value ? 'inline-block' : 'none'
        }

        if (searchTimeout) {
            clearTimeout(searchTimeout)
        }
        if (searchFrame) {
            cancelAnimationFrame(searchFrame)
        }

        // debounce
        searchTimeout = setTimeout(() => {
            searchFrame = requestAnimationFrame(() => {
                gridApi.setGridOption('quickFilterText', this.value)
                gridApi.setGridOption('loading', false)
            })
        }, 300)
    })

    if (clearButton) {
        clearButton.addEventListener('click', () => {
            inputElement.value = ''
            clearButton.style.display = 'none'
            inputElement.dispatchEvent(new Event('input'))
            inputElement.focus()
        })
    }
})

// filter reset button
function resetFilter() {
    const activeTab = document.querySelector('.active-tab-button').id
    let searchInput
    if (activeTab === 'records-button') {
        recordsGrid.setFilterModel(null)
        searchInput = document.getElementById('records-search-input')
    } else if (activeTab === 'manifest-button') {
        manifestGrid.setFilterModel(null)
        searchInput = document.getElementById('manifest-search-input')
    } else if (activeTab === 'log-button') {
        logTextGrid.setFilterModel(null)
        searchInput = document.getElementById('log-search-input')
    }
    if (searchInput) {
        searchInput.value = ''
        searchInput.dispatchEvent(new Event('input'))
    }
    updateFilterResetButtonState()
}

document.querySelector('#filter-reset-button').addEventListener('click', () => {
    resetFilter()
})

// reload button
document.querySelector('#reload-button').addEventListener('click', () => {
    window.location.reload()
})

// tab switching
recordsButton.addEventListener('click', () => {
    showTab('records')
})

manifestButton.addEventListener('click', () => {
    showTab('manifest')
})

logButton.addEventListener('click', () => {
    showTab('log')
})

function showTab(tabId) {
    const tabs = ['records', 'manifest', 'log']
    tabs.forEach(id => {
        const el = document.getElementById(id)
        if (el) {
            el.style.display = (id === tabId) ? 'block' : 'none'
        }

        const searchInput = document.getElementById(`${id}-search-input`)
        if (searchInput) {
            const inputContainer = searchInput.closest('.input-container')
            inputContainer.style.display = (id === tabId) ? 'block' : 'none'

            const clearButton = document.getElementById(`${id}-clear-button`)
            if (clearButton) {
                clearButton.style.display = (id === tabId && searchInput.value) ? 'inline-block' : 'none'
            }
        }
    })

    document.querySelectorAll('.tab-button').forEach(button => {
        button.classList.remove('active-tab-button')
    })

    document.getElementById(`${tabId}-button`).classList.add('active-tab-button')
    updateRowCount()
    updateFilterResetButtonState()
}

function updateRowCount() {
    const activeTab = document.querySelector('.active-tab-button').id
    let gridApi
    if (activeTab === 'records-button') {
        gridApi = recordsGrid
    } else if (activeTab === 'manifest-button') {
        gridApi = manifestGrid
    } else if (activeTab === 'log-button') {
        gridApi = logTextGrid
    }
    const count = gridApi ? gridApi.getDisplayedRowCount() : 0
    document.getElementById('row-count').textContent = count
}

// export as CSV
document.getElementById('csv-export-button').addEventListener('click', async () => {
    const activeTab = document.querySelector('.active-tab-button').id
    let gridApi, defaultName

    if (activeTab === 'records-button') {
        gridApi = recordsGrid
        defaultName = 'records.csv'
    } else if (activeTab === 'manifest-button') {
        gridApi = manifestGrid
        defaultName = 'manifest.csv'
    } else if (activeTab === 'log-button') {
        gridApi = logTextGrid
        defaultName = 'log.csv'
    }

    if (gridApi) {
        const csv = gridApi.getDataAsCsv()

        const filePath = await window.__TAURI__.dialog.save({
            defaultPath: defaultName,
            filters: [{ name: 'CSV', extensions: ['csv'] }]
        })

        if (filePath) {
            await window.__TAURI__.fs.writeTextFile(filePath, csv)
        }
    }
})

// tooltips
document.querySelectorAll('[data-tooltip]').forEach(btn => {
    let tooltipDiv
    let tooltipTimeout

    btn.addEventListener('mouseenter', () => {
        const text = btn.getAttribute('data-tooltip')
        if (!text) return

        tooltipTimeout = setTimeout(() => {
            tooltipDiv = document.createElement('div')
            tooltipDiv.textContent = text
            tooltipDiv.style.position = 'absolute'
            tooltipDiv.style.background = '#333'
            tooltipDiv.style.color = '#fff'
            tooltipDiv.style.padding = '6px 10px'
            tooltipDiv.style.borderRadius = '4px'
            tooltipDiv.style.fontSize = '13px'
            tooltipDiv.style.whiteSpace = 'nowrap'
            tooltipDiv.style.zIndex = '9999'
            tooltipDiv.style.pointerEvents = 'none'
            tooltipDiv.style.boxShadow = '0 2px 8px rgba(0,0,0,0.15)'
            tooltipDiv.style.textAlign = 'center'

            document.body.appendChild(tooltipDiv)

            // position below the button
            const rect = btn.getBoundingClientRect()
            tooltipDiv.style.top = `${rect.bottom + 6}px`
            tooltipDiv.style.left = `${rect.left + rect.width / 2}px`
            tooltipDiv.style.transform = 'translateX(-50%)'

            // prevent overflow
            const tipRect = tooltipDiv.getBoundingClientRect()
            if (tipRect.right > window.innerWidth) {
                tooltipDiv.style.left = `${window.innerWidth - tipRect.width / 2 - 8}px`
            }
            if (tipRect.left < 0) {
                tooltipDiv.style.left = `${tipRect.width / 2 + 8}px`
            }
        }, 500)
    })

    btn.addEventListener('mouseleave', () => {
        clearTimeout(tooltipTimeout)
        if (tooltipDiv) {
            tooltipDiv.remove()
            tooltipDiv = null
        }
    })
})