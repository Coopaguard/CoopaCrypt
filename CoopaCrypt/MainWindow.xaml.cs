using CoopaCrypt.Pops;
using Microsoft.Win32;
using System;
using System.IO;
using System.Linq;
using System.Text.RegularExpressions;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Input;

namespace CoopaCrypt
{
    /// <summary>
    /// Interaction logic for MainWindow.xaml
    /// </summary>
    public partial class MainWindow : Window
    {
        private PopFind? findWindow;

        public MainWindow()
        {
            InitializeComponent();
            var args = Environment.GetCommandLineArgs();

            if (args.Length > 0)
            {
                int i = 0;
                while (i < args.Length)
                {
                    if (File.Exists(args[i]) && new FileInfo(args[i]).Extension == ".coocrypt")
                    {
                        //open Dialog
                        ShowOpenDialog(args[i]);
                        break;
                    }
                    i++;
                }
            }

            this.LoadPosition("ViewWindow");

            Task.Run(() =>
            {
                FileAssociation.EnsureAssociationsSet();
            });
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            if (findWindow != null && findWindow.IsEnabled)
            {
                findWindow.Close();
            }

            this.SavePosition("ViewWindow");
        }


        #region actions

        private void NewAction_Click(object sender, RoutedEventArgs e)
        {
            this.Rtb.Text = "";
        }

        private void OpenAction_Click(object sender, RoutedEventArgs e)
        {
            var openFileDialog = new OpenFileDialog
            {
                Filter = "crypted files (*.coocrypt)|*.coocrypt|All files (*.*)|*.*",
                Multiselect = false
            };

            var fileSelected = openFileDialog.ShowDialog();
            if (fileSelected != null && fileSelected.Value)
            {
                ShowOpenDialog(openFileDialog.FileName);
            }
        }

        private void SaveAction_Click(object sender, RoutedEventArgs e)
        {
            var saveFileDialog = new OpenFileDialog
            {
                Filter = "crypted files (*.coocrypt)|*.coocrypt",
                Multiselect = false,
                AddExtension = true,
                CheckFileExists = false
            };

            var fileSelected = saveFileDialog.ShowDialog();
            if (fileSelected != null && fileSelected.Value)
            {
                var popPwd = new Pops.PopCrypto();
                popPwd.ShowDialog();

                if (popPwd.Pwd != null)
                {
                    try
                    {
                        Crypto.Crypt(saveFileDialog.FileName, this.Rtb.Text, popPwd.Pwd.ToString());
                    }
                    catch (Exception ex)
                    {
                        MessageBox.Show($"Erreur lors du cryptage: {ex.Message}", "Erreur", MessageBoxButton.OK, MessageBoxImage.Error);
                    }
                }
            }
        }

        private void FindAction_Click(object sender, RoutedEventArgs e)
        {
            ShowFind();
        }

        private void ReplaceAction_Click(object sender, RoutedEventArgs e)
        {
            ShowFind(true);
        }

        private void UndoAction_Click(object sender, RoutedEventArgs e)
        {
            Rtb.Undo();
        }

        private void RedoAction_Click(object sender, RoutedEventArgs e)
        {
            Rtb.Redo();
        }

        private void Window_KeyDown(object sender, KeyEventArgs e)
        {
            if (e.Key == Key.F && Keyboard.Modifiers == ModifierKeys.Control)
            {
                FindAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.R && Keyboard.Modifiers == ModifierKeys.Control)
            {
                ReplaceAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.O && Keyboard.Modifiers == ModifierKeys.Control)
            {
                OpenAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.S && Keyboard.Modifiers == ModifierKeys.Control)
            {
                SaveAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.E && Keyboard.Modifiers == ModifierKeys.Control)
            {
                RedoAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.Z && Keyboard.Modifiers == ModifierKeys.Control)
            {
                UndoAction_Click(sender, new RoutedEventArgs());
            }

            if (e.Key == Key.F3 && findWindow != null && findWindow.Focusable)
            {
                findWindow.BtnFind_Click(sender, e);
            }
        }

        #endregion

        #region privates methods

        private void ShowOpenDialog(string filePath)
        {
            var popPwd = new Pops.PopCrypto();
            popPwd.ShowDialog();

            if (popPwd.Pwd != null)
            {
                try
                {
                    var decrypted = Crypto.Decrypt(filePath, popPwd.Pwd.ToString());
                    this.Rtb.Text = decrypted;
                }
                catch (Exception ex)
                {
                    MessageBox.Show($"Erreur lors du décryptage: {ex.Message}", "Erreur", MessageBoxButton.OK, MessageBoxImage.Error);
                }
            }
        }

        private void ShowFind(bool Onreplace = false)
        {
            var selectedText = Rtb.SelectedText;

            if (findWindow == null || !findWindow.IsActive)
            {
                findWindow = new PopFind(this, selectedText, Onreplace);
                findWindow.Show();
            }
            else
            {
                findWindow.Show();
                findWindow.Focus();
                findWindow.Init(selectedText, Onreplace);
            }
        }

        public void FindNext(string text, bool caseSensitive)
        {
            var currentPos = Rtb.SelectionStart;
            MatchCollection match;

            if (caseSensitive)
            {
                match = Regex.Matches(Rtb.Text, text, RegexOptions.Multiline);
            }
            else
            {
                match = Regex.Matches(Rtb.Text, text, RegexOptions.IgnoreCase | RegexOptions.Multiline);
            }

            if (match.FirstOrDefault(m => m.Success && m.Index > currentPos) is var m && m is not null)
            {
                Rtb.SelectionStart = m.Index;
                Rtb.SelectionLength = m.Length;
                Rtb.Focus();
            }
            else
            {
                Rtb.SelectionLength = 0;
                Rtb.SelectionStart = 0;

                MessageBox.Show("Fin du document atteind", "Plus d'index trouvé", MessageBoxButton.OK, MessageBoxImage.Information);
            }
        }

        public void ReplaceNext(string find, bool caseSensitive, string replace)
        {
            FindNext(find, caseSensitive);

            if (!string.IsNullOrWhiteSpace(Rtb.SelectedText))
            {
                Rtb.SelectedText = replace;
            }
        }

        public void ReplaceAll(string find, bool caseSensitive, string replace)
        {
            MatchCollection match;

            if (caseSensitive)
            {
                match = Regex.Matches(Rtb.Text, find, RegexOptions.Multiline);
            }
            else
            {
                match = Regex.Matches(Rtb.Text, find, RegexOptions.IgnoreCase | RegexOptions.Multiline);
            }

            foreach (var m in match.Select(m => m.Value).Distinct())
            {
                Rtb.Text = Rtb.Text.Replace(m, replace);
            }

            MessageBox.Show($"{match.Count} valeurs rempalcés", "Remplacement", MessageBoxButton.OK, MessageBoxImage.Asterisk);
        }

        #endregion

    }
}
