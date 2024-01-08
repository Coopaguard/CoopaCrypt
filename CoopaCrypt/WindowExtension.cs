using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace CoopaCrypt
{
    public static class WindowExtension
    {
        public static void LoadPosition(this Window window, string Name)
        {
            var fileName = Name + "-config.json";

            if (File.Exists(fileName))
            {
                var cfg = System.Text.Json.JsonSerializer.Deserialize<WindowConfig>(File.ReadAllText(fileName));

                if(cfg.X < SystemParameters.VirtualScreenWidth && cfg.Y < Math.Abs(SystemParameters.VirtualScreenTop))
                {
                    window.Top = cfg.Y;
                    window.Left = cfg.X;
                }

                window.Height = cfg.Height;
                window.Width = cfg.Width;
                window.Opacity = cfg.Opacity;
            }
        }
        public static void SavePosition(this Window window, string Name)
        {
            var fileName = Name + "-config.json";

            var pos = window.Top;
            var cfg = new WindowConfig
            {
                Width = window.Width,
                Height = window.Height,
                X = window.Left,
                Y = window.Top,
                Opacity = window.Opacity,
            };

            File.WriteAllText(fileName, System.Text.Json.JsonSerializer.Serialize(cfg));
        }
    }

    public class WindowConfig
    {
        public double X { get; set; }

        public double Y { get; set; }

        public double Width { get; set; }

        public double Height { get; set; }

        public double Opacity { get; set; }
    }
}
