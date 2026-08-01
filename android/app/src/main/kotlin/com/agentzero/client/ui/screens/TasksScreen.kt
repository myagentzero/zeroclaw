package com.agentzero.client.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Checklist
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.agentzero.client.AppContainer
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.data.model.TaskItem
import com.agentzero.client.ui.components.EmptyState
import com.agentzero.client.ui.components.ErrorBanner
import com.agentzero.client.ui.components.LoadingState
import com.agentzero.client.ui.theme.Spacing
import com.agentzero.client.ui.util.formatIsoDateTime
import kotlinx.coroutines.launch

private val STATUS_FILTERS = listOf("all", "pending", "in_progress", "completed")

private fun statusLabel(status: String): String = when (status) {
    "in_progress" -> "In Progress"
    "completed" -> "Completed"
    "pending" -> "Pending"
    else -> status.replaceFirstChar { it.uppercase() }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun TasksScreen(config: ServerConfig, container: AppContainer) {
    var tasks by remember { mutableStateOf<List<TaskItem>>(emptyList()) }
    var loading by remember { mutableStateOf(true) }
    var refreshing by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    var filter by remember { mutableStateOf("all") }
    var selected by remember { mutableStateOf<TaskItem?>(null) }
    var confirmDelete by remember { mutableStateOf<TaskItem?>(null) }
    val scope = rememberCoroutineScope()

    fun reload(refresh: Boolean = false) {
        scope.launch {
            if (refresh) refreshing = true else loading = true
            error = null
            runCatching { tasks = container.gatewayClient.getTasks(config) }
                .onFailure { error = it.message }
            loading = false
            refreshing = false
        }
    }

    LaunchedEffect(config) { reload() }

    val filtered = if (filter == "all") tasks else tasks.filter { it.status == filter }

    Scaffold { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(Spacing.md),
            verticalArrangement = Arrangement.spacedBy(Spacing.sm),
        ) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "Tasks (${filtered.size})",
                    style = MaterialTheme.typography.titleMedium,
                )
                IconButton(onClick = { reload(refresh = true) }, enabled = !refreshing) {
                    Icon(Icons.Default.Refresh, contentDescription = "Refresh")
                }
            }

            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(Spacing.xs),
            ) {
                STATUS_FILTERS.forEach { status ->
                    FilterChip(
                        selected = filter == status,
                        onClick = { filter = status },
                        label = { Text(statusLabel(status)) },
                    )
                }
            }

            error?.let { ErrorBanner(it, onRetry = { reload() }) }

            when {
                loading -> LoadingState()

                filtered.isEmpty() -> EmptyState(
                    icon = Icons.Default.Checklist,
                    title = "No tasks found.",
                )

                else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                    items(filtered, key = { it.id }) { task ->
                        Card(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { selected = task },
                        ) {
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .padding(Spacing.md),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.Top,
                            ) {
                                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(Spacing.xs)) {
                                    Text(
                                        task.subject,
                                        style = MaterialTheme.typography.titleSmall,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                    Row(
                                        horizontalArrangement = Arrangement.spacedBy(Spacing.sm),
                                        verticalAlignment = Alignment.CenterVertically,
                                    ) {
                                        TaskStatusChip(task.status)
                                        if (task.blocked) {
                                            Icon(
                                                Icons.Default.Lock,
                                                contentDescription = "Blocked",
                                                modifier = Modifier.size(14.dp),
                                                tint = MaterialTheme.colorScheme.error,
                                            )
                                        }
                                    }
                                    task.owner?.takeIf { it.isNotBlank() }?.let {
                                        Text(
                                            "Owner: $it",
                                            style = MaterialTheme.typography.labelSmall,
                                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                                        )
                                    }
                                    Text(
                                        "Updated: ${formatIsoDateTime(task.updatedAt)}",
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                                IconButton(onClick = { confirmDelete = task }) {
                                    Icon(
                                        Icons.Default.Delete,
                                        contentDescription = "Delete task",
                                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    selected?.let { task ->
        AlertDialog(
            onDismissRequest = { selected = null },
            confirmButton = { TextButton(onClick = { selected = null }) { Text("Close") } },
            title = { Text(task.subject) },
            text = {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(max = 420.dp)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(Spacing.sm),
                ) {
                    DetailLine("ID", task.id)
                    DetailLine("Description", task.description.ifBlank { "—" })
                    task.activeForm?.takeIf { it.isNotBlank() }?.let { DetailLine("Active Form", it) }
                    DetailLine("Status", statusLabel(task.status))
                    DetailLine("Owner", task.owner ?: "—")
                    DetailLine(
                        "Blocked By",
                        task.blockedBy.takeIf { it.isNotEmpty() }?.joinToString(", ") ?: "None",
                        monospace = true,
                    )
                    DetailLine(
                        "Blocks",
                        task.blocks.takeIf { it.isNotEmpty() }?.joinToString(", ") ?: "None",
                        monospace = true,
                    )
                    if (task.metadata.isNotEmpty()) {
                        DetailLine("Metadata", task.metadata.toString(), monospace = true)
                    }
                    DetailLine("Created", formatIsoDateTime(task.createdAt))
                    DetailLine("Updated", formatIsoDateTime(task.updatedAt))
                }
            },
        )
    }

    confirmDelete?.let { task ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text("Delete task?") },
            text = { Text("Delete \"${task.subject}\"?") },
            confirmButton = {
                TextButton(onClick = {
                    scope.launch {
                        runCatching { container.gatewayClient.deleteTask(config, task.id) }
                            .onSuccess {
                                tasks = tasks.filter { it.id != task.id }
                                confirmDelete = null
                            }
                            .onFailure {
                                error = it.message
                                confirmDelete = null
                            }
                    }
                }) { Text("Delete") }
            },
            dismissButton = { TextButton(onClick = { confirmDelete = null }) { Text("Cancel") } },
        )
    }
}

@Composable
private fun TaskStatusChip(status: String, modifier: Modifier = Modifier) {
    val containerColor = when (status) {
        "completed" -> MaterialTheme.colorScheme.tertiaryContainer
        "in_progress" -> MaterialTheme.colorScheme.primaryContainer
        else -> MaterialTheme.colorScheme.surfaceVariant
    }
    val contentColor = when (status) {
        "completed" -> MaterialTheme.colorScheme.onTertiaryContainer
        "in_progress" -> MaterialTheme.colorScheme.onPrimaryContainer
        else -> MaterialTheme.colorScheme.onSurfaceVariant
    }
    Surface(modifier = modifier, color = containerColor, shape = MaterialTheme.shapes.small) {
        Text(
            statusLabel(status),
            modifier = Modifier.padding(horizontal = Spacing.sm, vertical = 2.dp),
            style = MaterialTheme.typography.labelSmall,
            color = contentColor,
        )
    }
}

@Composable
private fun DetailLine(label: String, value: String, monospace: Boolean = false) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(
            value,
            style = MaterialTheme.typography.bodySmall,
            fontFamily = if (monospace) FontFamily.Monospace else FontFamily.Default,
            maxLines = if (monospace) Int.MAX_VALUE else 4,
            overflow = if (monospace) TextOverflow.Visible else TextOverflow.Ellipsis,
        )
    }
}
