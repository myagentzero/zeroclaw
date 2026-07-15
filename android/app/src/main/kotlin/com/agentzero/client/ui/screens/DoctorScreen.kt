package com.agentzero.client.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.CheckCircle
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.MedicalServices
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.agentzero.client.AppContainer
import com.agentzero.client.data.model.DiagResult
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.ui.components.EmptyState
import com.agentzero.client.ui.components.ErrorBanner
import com.agentzero.client.ui.theme.Spacing
import kotlinx.coroutines.launch

private fun severityIcon(severity: String): ImageVector = when (severity) {
    "ok" -> Icons.Default.CheckCircle
    "warn" -> Icons.Default.Warning
    else -> Icons.Default.Error
}

private fun severityColor(severity: String): Color = when (severity) {
    "ok" -> Color(0xFF4CAF50)
    "warn" -> Color(0xFFFFC107)
    else -> Color(0xFFF44336)
}

@Composable
fun DoctorScreen(config: ServerConfig, container: AppContainer) {
    var results by remember { mutableStateOf<List<DiagResult>?>(null) }
    var loading by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    fun run() {
        scope.launch {
            loading = true
            error = null
            results = null
            runCatching { container.gatewayClient.runDoctor(config) }
                .onSuccess { results = it }
                .onFailure { error = it.message ?: "Failed to run diagnostics" }
            loading = false
        }
    }

    LaunchedEffect(config) { run() }

    val okCount = results?.count { it.severity == "ok" } ?: 0
    val warnCount = results?.count { it.severity == "warn" } ?: 0
    val errorCount = results?.count { it.severity == "error" } ?: 0
    val grouped = results?.groupBy { it.category }?.toSortedMap() ?: emptyMap()

    Column(
        Modifier
            .fillMaxSize()
            .padding(Spacing.md),
        verticalArrangement = Arrangement.spacedBy(Spacing.sm),
    ) {
        Row(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Diagnostics", style = MaterialTheme.typography.titleMedium)
            Button(onClick = { run() }, enabled = !loading) {
                if (loading) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(16.dp),
                        strokeWidth = 2.dp,
                        color = MaterialTheme.colorScheme.onPrimary,
                    )
                } else {
                    Icon(Icons.Default.PlayArrow, contentDescription = null, modifier = Modifier.size(18.dp))
                }
                Text(if (loading) " Running..." else " Run Diagnostics")
            }
        }

        error?.let { ErrorBanner(it, onRetry = { run() }) }

        when {
            loading && results == null -> Column(
                Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                CircularProgressIndicator()
                Text(
                    "Running diagnostics...",
                    modifier = Modifier.padding(top = Spacing.md),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            results == null && error == null -> EmptyState(
                icon = Icons.Default.MedicalServices,
                title = "System Diagnostics",
                subtitle = "Tap \"Run Diagnostics\" to check your AgentZero installation.",
            )

            results != null -> LazyColumn(verticalArrangement = Arrangement.spacedBy(Spacing.md)) {
                item {
                    Card(Modifier.fillMaxWidth()) {
                        Row(
                            Modifier
                                .fillMaxWidth()
                                .padding(Spacing.md),
                            horizontalArrangement = Arrangement.spacedBy(Spacing.lg),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            SummaryCount(Icons.Default.CheckCircle, severityColor("ok"), okCount, "ok")
                            SummaryCount(Icons.Default.Warning, severityColor("warn"), warnCount, "warnings")
                            SummaryCount(Icons.Default.Error, severityColor("error"), errorCount, "errors")
                        }
                    }
                }

                grouped.forEach { (category, categoryResults) ->
                    item {
                        Text(
                            category.uppercase(),
                            style = MaterialTheme.typography.labelMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = Spacing.sm),
                        )
                    }
                    items(categoryResults) { result ->
                        Card(
                            Modifier.fillMaxWidth(),
                            colors = CardDefaults.cardColors(
                                containerColor = severityColor(result.severity).copy(alpha = 0.08f),
                            ),
                        ) {
                            Row(
                                Modifier.padding(Spacing.md),
                                verticalAlignment = Alignment.Top,
                                horizontalArrangement = Arrangement.spacedBy(Spacing.sm),
                            ) {
                                Icon(
                                    severityIcon(result.severity),
                                    contentDescription = result.severity,
                                    tint = severityColor(result.severity),
                                    modifier = Modifier.size(20.dp),
                                )
                                Column {
                                    Text(result.message, style = MaterialTheme.typography.bodyMedium)
                                    Text(
                                        result.severity,
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun SummaryCount(icon: ImageVector, color: Color, count: Int, label: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.xs)) {
        Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(18.dp))
        Text("$count", style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.Bold)
        Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
