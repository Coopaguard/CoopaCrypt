using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace CoopaCrypt
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        string npdRegistry = "Applications\\notepad++.exe\\shell\\open\\command";
        public MainWindow()
        {
            InitializeComponent();
            Environment.GetCommandLineArgs();


        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            string? path = null;
            try
            {
                path = GetNotepadPath();
            }
            catch(Exception ex)
            {
                MessageBox.Show("Merci d'installer Notepad++", "Error", MessageBoxButton.OK, MessageBoxImage.Error);
            }

            if (path != null)
            {
                var args = "-multiInst -nosession -notabbar -alwaysOnTop -qSpeed3 -qt=\"demo content\"";
                var current = Process.Start(path, args);
                current.Exited += Current_Exited;
                current.WaitForExit();
            }
        }

        private void Current_Exited(object? sender, EventArgs e)
        {
            MessageBox.Show(e.ToString(), sender.ToString());
        }

        private string? GetNotepadPath()
        {
            return Registry.ClassesRoot.OpenSubKey(npdRegistry)?.GetValue("")?.ToString().Split('"')[1];
        }
    }
}
