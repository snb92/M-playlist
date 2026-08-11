using System;
using System.Globalization;
using System.Windows.Data;

namespace MPlaylistApp
{
    public class TimecodeConverter : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is ulong hnsecs)
            {
                TimeSpan t = TimeSpan.FromTicks((long)hnsecs);
                int frames = (int)(t.Milliseconds / 33.33); // Assuming 30fps
                return $"{t.Hours:D2}:{t.Minutes:D2}:{t.Seconds:D2}:{frames:D2}";
            }
            if (value is long hnsecsLong)
            {
                TimeSpan t = TimeSpan.FromTicks(hnsecsLong);
                int frames = (int)(t.Milliseconds / 33.33);
                return $"{t.Hours:D2}:{t.Minutes:D2}:{t.Seconds:D2}:{frames:D2}";
            }
            if (value is double durDouble)
            {
                TimeSpan t = TimeSpan.FromSeconds(durDouble);
                int frames = (int)(t.Milliseconds / 33.33);
                return $"{t.Hours:D2}:{t.Minutes:D2}:{t.Seconds:D2}:{frames:D2}";
            }
            return value;
        }

        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        {
            if (value is string s)
            {
                var parts = s.Split(':');
                if (parts.Length == 4)
                {
                    if (int.TryParse(parts[0], out int h) &&
                        int.TryParse(parts[1], out int m) &&
                        int.TryParse(parts[2], out int sec) &&
                        int.TryParse(parts[3], out int f))
                    {
                        double totalSeconds = h * 3600 + m * 60 + sec + (f * 33.33 / 1000.0);
                        if (targetType == typeof(ulong)) return (ulong)(totalSeconds * 10_000_000.0);
                        if (targetType == typeof(long)) return (long)(totalSeconds * 10_000_000.0);
                        if (targetType == typeof(double)) return totalSeconds;
                    }
                }
            }
            return 0; // fallback
        }
    }
}
