from datetime import date, timedelta, datetime
import math

class SubsidyPredictor:
    """
    Predicts the next announcement date for recurring government subsidies.
    Based on the 'Recurring Event Prediction' logic from the technical report.
    """

    def __init__(self):
        # 2026 Lunar New Year (Seollal) is Feb 17, 2026.
        # Holidays usually delay announcements.
        self.holidays_2026 = [
            date(2026, 1, 1),   # New Year
            date(2026, 2, 17),  # Seollal
            date(2026, 2, 18),
            date(2026, 2, 19),
            date(2026, 3, 1),   # Independence Day
        ]

    def predict_next_date(self, history_dates: list[date]) -> dict:
        """
        Predicts the 2026 announcement date based on historical dates (2024, 2025).
        Formula: Weighted Average (w2 > w1)
        """
        if not history_dates:
            return {"predicted_date": None, "confidence": "Low", "reason": "No history data"}

        # Sort history to identify years
        history_dates.sort()
        
        # Convert dates to "Day of Year" (1-365)
        doys = []
        for d in history_dates:
            doy = d.timetuple().tm_yday
            doys.append(doy)
            
        # Weighted Average Logic
        # If we have 2 years (2024, 2025), weight 2025 more.
        if len(doys) >= 2:
            w1 = 0.3  # Older (2024)
            w2 = 0.7  # Recent (2025)
            # Use the last two data points
            predicted_doy = (doys[-2] * w1) + (doys[-1] * w2)
        else:
            # Only 1 year data
            predicted_doy = doys[-1]

        predicted_doy = int(predicted_doy)
        
        # Convert back to 2026 date
        predicted_date_2026 = date(2026, 1, 1) + timedelta(days=predicted_doy - 1)
        
        # Adjustment: Avoid weekends and holidays
        predicted_date_2026 = self._adjust_for_business_day(predicted_date_2026)

        # Create an interval (e.g., +/- 5 days)
        start_range = predicted_date_2026 - timedelta(days=5)
        end_range = predicted_date_2026 + timedelta(days=5)

        return {
            "predicted_date": predicted_date_2026.isoformat(),
            "range_start": start_range.isoformat(),
            "range_end": end_range.isoformat(),
            "confidence": "Medium" if len(history_dates) >= 2 else "Low",
            "reason": "Based on weighted average of previous years"
        }

    def _adjust_for_business_day(self, target_date: date) -> date:
        """
        If target_date is weekend or holiday, move to next business day.
        """
        while target_date.weekday() >= 5 or target_date in self.holidays_2026:
            target_date += timedelta(days=1)
        return target_date

if __name__ == "__main__":
    # Test based on report example
    # 2024: Feb 15 (DOY 46)
    # 2025: Feb 20 (DOY 51)
    # Expected: 2026 Prediction around DOY 49-50 (Feb 18-19), adjusted for Seollal (Feb 17-19)
    
    predictor = SubsidyPredictor()
    history = [date(2024, 2, 15), date(2025, 2, 20)]
    
    result = predictor.predict_next_date(history)
    print(f"History: {history}")
    print(f"Prediction: {result}")
