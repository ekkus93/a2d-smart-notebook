package com.a2d.notebook.rustbridge

import com.sun.jna.Pointer
import com.sun.jna.Structure

@Structure.FieldOrder("capacity", "len", "data")
internal open class PreviewProcessingBuffer : Structure() {
    @JvmField var capacity: Long = 0L
    @JvmField var len: Long = 0L
    @JvmField var data: Pointer? = null

    class ByValue : PreviewProcessingBuffer(), Structure.ByValue
}

@Structure.FieldOrder("code", "error")
internal open class PreviewProcessingStatus : Structure() {
    @JvmField var code: Int = 0
    @JvmField var error: PreviewProcessingBuffer.ByValue = PreviewProcessingBuffer.ByValue()
}
