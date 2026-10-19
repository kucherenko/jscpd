package com.example.loans

import kotlin.math.pow
import kotlin.math.roundToLong

object Interest {
    /** The monthly payment of an annuity loan, in cents. */
    fun monthlyPayment(principalCents: Long, annualRatePercent: Double, months: Int): Long {
        if (months <= 0) return principalCents
        val monthlyRate = annualRatePercent / 100.0 / 12.0
        if (monthlyRate == 0.0) return principalCents / months
        val factor = (1 + monthlyRate).pow(months)
        val payment = principalCents * monthlyRate * factor / (factor - 1)
        return payment.roundToLong()
    }

    /** "4.5%" or "4%": how the rate reads on a statement. */
    fun formatRate(annualRatePercent: Double): String {
        val hundredths = (annualRatePercent * 100).roundToLong()
        val whole = hundredths / 100
        val rest = hundredths % 100
        return when {
            rest == 0L -> "$whole%"
            rest % 10 == 0L -> "$whole.${rest / 10}%"
            else -> "$whole.${rest.toString().padStart(2, '0')}%"
        }
    }
}
