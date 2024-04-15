using System;
using System.Diagnostics;
using System.IO;
using System.Windows;
using System.Windows.Shapes;

namespace CoopaCrypt
{
    public static class WindowExtension
    {
        private static string _exPath => new FileInfo(System.Reflection.Assembly.GetExecutingAssembly().Location).Directory?.FullName ?? string.Empty;

        public static void LoadPosition(this Window window, string Name)
        {
            var fileName = Name + "-config.json";

            if (File.Exists(System.IO.Path.Combine(_exPath, fileName)))
            {
                var cfg = System.Text.Json.JsonSerializer.Deserialize<WindowConfig>(File.ReadAllText(System.IO.Path.Combine(_exPath, fileName)));

                if(cfg != null)
                {

                    if (cfg.X < SystemParameters.VirtualScreenWidth && cfg.Y < Math.Abs(SystemParameters.VirtualScreenHeight))
                    {
                        window.Top = cfg.Y;
                        window.Left = cfg.X;
                    }

                    window.Height = cfg.Height;
                    window.Width = cfg.Width;
                    window.Opacity = cfg.Opacity;
                }
            }
        }
        public static void SavePosition(this Window window, string Name)
        {
            var fileName = Name + "-config.json";
            var cfg = new WindowConfig
            {
                Width = window.Width,
                Height = window.Height,
                X = window.Left,
                Y = window.Top,
                Opacity = window.Opacity,
            };

            File.WriteAllText(System.IO.Path.Combine(_exPath, fileName), System.Text.Json.JsonSerializer.Serialize(cfg));
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
