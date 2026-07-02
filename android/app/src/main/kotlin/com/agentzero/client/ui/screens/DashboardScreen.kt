package com.agentzero.client.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.EventNote
import androidx.compose.material.icons.filled.Bolt
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Memory
import androidx.compose.material.icons.filled.Schedule
import androidx.compose.material.icons.filled.Storage
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.agentzero.client.AppContainer
import com.agentzero.client.data.model.CostSummary
import com.agentzero.client.data.model.CronJob
import com.agentzero.client.data.model.ServerConfig
import com.agentzero.client.data.model.StatusResponse
import com.agentzero.client.ui.components.ErrorState
import com.agentzero.client.ui.components.LoadingState
import com.agentzero.client.ui.components.StatusChip
import com.agentzero.client.ui.theme.Spacing
import java.time.Instant
import java.util.Locale

@Composable
fun DashboardScreen(config: ServerConfig, container: AppContainer) {
    var status by remember { mutableStateOf<StatusResponse?>(null) }
    var cost by remember { mutableStateOf<CostSummary?>(null) }
    var upcomingJobs by remember { mutableStateOf(0) }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(true) }
    var retryToken by remember { mutableIntStateOf(0) }

    LaunchedEffect(config, retryToken) {
        loading = true
        error = null
        runCatching {
            val s = container.gatewayClient.getStatus(config)
            val c = container.gatewayClient.getCost(config)
            status = s
            cost = c
        }.onFailure { error = it.message }
        runCatching {
            upcomingJobs = countUpcomingJobs(container.gatewayClient.getCronJobs(config))
        }
        loading = false
    }

    when {
        loading -> LoadingState()
        error != null -> ErrorState(error ?: "Something went wrong.", onRetry = { retryToken++ })
        status != null && cost != null -> DashboardContent(status!!, cost!!, upcomingJobs)
    }
}

private fun countUpcomingJobs(jobs: List<CronJob>): Int {
    val now = Instant.now()
    return jobs.count { job ->
        job.enabled && runCatching { Instant.parse(job.nextRun).isAfter(now) }.getOrDefault(false)
    }
}

@Composable
private fun DashboardContent(status: StatusResponse, cost: CostSummary, upcomingJobs: Int) {
    val maxCost = maxOf(cost.hourlyCostUsd, cost.dailyCostUsd, cost.monthlyCostUsd, 0.001)
    var costOpen by remember { mutableStateOf(true) }
    var tokensOpen by remember { mutableStateOf(true) }
    var healthOpen by remember { mutableStateOf(true) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(Spacing.lg),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        Column {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text("Runtime Dashboard", style = MaterialTheme.typography.headlineSmall)
                Spacer(Modifier.size(Spacing.sm))
                StatusChip(if (status.paired) "Paired" else "Unpaired", active = status.paired)
            }
            Text(
                "v${status.version}",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(Spacing.sm)) {
            MetricCard(Icons.Default.Bolt, "Provider", status.provider ?: "Unknown", Modifier.weight(1f))
            MetricCard(Icons.Default.Memory, "Model", status.model, Modifier.weight(1f))
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(Spacing.sm)) {
            MetricCard(Icons.Default.Schedule, "Uptime", formatUptime(status.uptimeSeconds), Modifier.weight(1f))
            MetricCard(Icons.AutoMirrored.Filled.EventNote, "Scheduled Jobs", "$upcomingJobs", Modifier.weight(1f))
        }
        MetricCard(
            Icons.Default.Storage,
            "Memory Backend",
            status.memoryBackend.replaceFirstChar {
                if (it.isLowerCase()) it.titlecase(Locale.getDefault()) else it.toString()
            },
            Modifier.fillMaxWidth(),
        )

        CollapsibleCard("Cost Pulse", costOpen, { costOpen = !costOpen }) {
            listOf(
                "Hourly" to cost.hourlyCostUsd,
                "Daily" to cost.dailyCostUsd,
                "Monthly" to cost.monthlyCostUsd,
            ).forEach { (label, value) ->
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(label, style = MaterialTheme.typography.bodyMedium)
                    Text(formatUsd(value), fontWeight = FontWeight.SemiBold)
                }
                LinearProgressIndicator(
                    progress = { (value / maxCost).toFloat().coerceIn(0.03f, 1f) },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(top = 4.dp, bottom = Spacing.xs),
                )
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text("Total Tokens: ${cost.totalTokens}", style = MaterialTheme.typography.bodySmall)
                Text("Requests: ${cost.requestCount}", style = MaterialTheme.typography.bodySmall)
            }
        }

        CollapsibleCard("Token Statistics (24h)", tokensOpen, { tokensOpen = !tokensOpen }) {
            val avg = if (cost.requestCount > 0) cost.totalTokens / cost.requestCount else 0
            val costPer1k = if (cost.totalTokens > 0) {
                (cost.dailyCostUsd / cost.totalTokens) * 1000
            } else 0.0
            LabeledStat("Total Tokens", "${cost.totalTokens}")
            LabeledStat("Avg Tokens / Request", "$avg")
            LabeledStat("Cost per 1K Tokens", formatUsd(costPer1k))
        }

        CollapsibleCard("Component Health", healthOpen, { healthOpen = !healthOpen }) {
            if (status.health.components.isEmpty()) {
                Text(
                    "No component health is currently available.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                status.health.components.forEach { (name, component) ->
                    val healthy = component.status.equals("healthy", ignoreCase = true) ||
                        component.status.equals("ok", ignoreCase = true)
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(
                            modifier = Modifier
                                .size(8.dp)
                                .background(
                                    if (healthy) MaterialTheme.colorScheme.tertiary else MaterialTheme.colorScheme.error,
                                    CircleShape,
                                ),
                        )
                        Column(Modifier.padding(start = Spacing.sm).weight(1f)) {
                            Text(
                                name.replaceFirstChar {
                                    if (it.isLowerCase()) it.titlecase(Locale.getDefault()) else it.toString()
                                },
                                fontWeight = FontWeight.Medium,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            Text(
                                component.status + if (component.restartCount > 0) " · Restarts: ${component.restartCount}" else "",
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

@Composable
private fun LabeledStat(label: String, value: String) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
        Text(label, style = MaterialTheme.typography.bodyMedium)
        Text(value, fontWeight = FontWeight.SemiBold)
    }
}

@Composable
private fun MetricCard(icon: ImageVector, title: String, value: String, modifier: Modifier = Modifier) {
    Card(modifier = modifier, colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant)) {
        Column(Modifier.padding(Spacing.md)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    icon,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp),
                    tint = MaterialTheme.colorScheme.primary,
                )
                Text(
                    title,
                    style = MaterialTheme.typography.labelSmall,
                    modifier = Modifier.padding(start = Spacing.xs),
                )
            }
            Spacer(Modifier.height(Spacing.xs))
            Text(value, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        }
    }
}

@Composable
private fun CollapsibleCard(
    title: String,
    expanded: Boolean,
    onToggle: () -> Unit,
    content: @Composable () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(Spacing.md)) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                IconButton(onClick = onToggle) {
                    Icon(
                        if (expanded) Icons.Default.ExpandLess else Icons.Default.ExpandMore,
                        contentDescription = null,
                    )
                }
            }
            AnimatedVisibility(expanded) {
                Column(Modifier.padding(top = Spacing.xs), verticalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                    content()
                }
            }
        }
    }
}

private fun formatUptime(seconds: Long): String {
    val d = seconds / 86400
    val h = (seconds % 86400) / 3600
    val m = (seconds % 3600) / 60
    return when {
        d > 0 -> "${d}d ${h}h ${m}m"
        h > 0 -> "${h}h ${m}m"
        else -> "${m}m"
    }
}

private fun formatUsd(value: Double): String = String.format(Locale.US, "$%.4f", value)
