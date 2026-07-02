package com.agentzero.client.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

private val LightColors = lightColorScheme(
    primary = Color(0xFF1A56DB),
    onPrimary = Color.White,
    primaryContainer = Color(0xFFDBE6FF),
    onPrimaryContainer = Color(0xFF001A43),
    secondary = Color(0xFF4F83FF),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFE1E9FF),
    onSecondaryContainer = Color(0xFF0E1B3B),
    tertiary = Color(0xFF0D9488),
    onTertiary = Color.White,
    background = Color(0xFFF8F9FC),
    onBackground = Color(0xFF1A1C1E),
    surface = Color(0xFFF8F9FC),
    onSurface = Color(0xFF1A1C1E),
    surfaceVariant = Color(0xFFE4E8F0),
    onSurfaceVariant = Color(0xFF44474E),
)

private val DarkColors = darkColorScheme(
    primary = Color(0xFF7EA5EB),
    onPrimary = Color(0xFF06254B),
    primaryContainer = Color(0xFF1E3A66),
    onPrimaryContainer = Color(0xFFDBE6FF),
    secondary = Color(0xFF9DB8FF),
    onSecondary = Color(0xFF17265A),
    secondaryContainer = Color(0xFF283A72),
    onSecondaryContainer = Color(0xFFE1E9FF),
    tertiary = Color(0xFF2DD4BF),
    onTertiary = Color(0xFF00382F),
    background = Color(0xFF10131A),
    onBackground = Color(0xFFE3E2E6),
    surface = Color(0xFF10131A),
    onSurface = Color(0xFFE3E2E6),
    surfaceVariant = Color(0xFF262A33),
    onSurfaceVariant = Color(0xFFC4C6D0),
)

private val AppShapes = Shapes(
    extraSmall = RoundedCornerShape(6.dp),
    small = RoundedCornerShape(10.dp),
    medium = RoundedCornerShape(14.dp),
    large = RoundedCornerShape(20.dp),
    extraLarge = RoundedCornerShape(28.dp),
)

private val AppTypography = Typography().let { base ->
    base.copy(
        headlineSmall = base.headlineSmall.copy(fontWeight = FontWeight.SemiBold),
        titleLarge = base.titleLarge.copy(fontWeight = FontWeight.SemiBold),
        titleMedium = base.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        titleSmall = base.titleSmall.copy(fontWeight = FontWeight.SemiBold),
    )
}

@Composable
fun AgentZeroTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (isSystemInDarkTheme()) DarkColors else LightColors,
        typography = AppTypography,
        shapes = AppShapes,
        content = content,
    )
}
