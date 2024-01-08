using System;
using System.Collections.Generic;
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
using System.Windows.Shapes;

namespace CoopaCrypt.Pops
{
    /// <summary>
    /// Interaction logic for PopFind.xaml
    /// </summary>
    public partial class PopFind : Window
    {
        private readonly MainWindow _mainWindow;

        public PopFind(MainWindow mainWindow, string? selectedText, bool onReplace = false)
        {
            InitializeComponent();
            _mainWindow = mainWindow;
            Init(selectedText, onReplace);

            this.LoadPosition("PopFind");

            //SOpacity.Value = this.Opacity;
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            this.SavePosition("PopFind");
        }

        public void Init(string? selectedText, bool onReplace = false)
        {
            TbFind.Text = selectedText;
            if (!onReplace)
            {
                TbFind.Focus();
            }
            else
            {
                TbReplace.Focus();
            }
        }

        public void BtnFind_Click(object sender, RoutedEventArgs e)
        {
            _mainWindow.FindNext(
                TbFind.Text.Trim(), 
                CbCaseSensitive.IsChecked != null && CbCaseSensitive.IsChecked.Value);
        }

        private void BtnReplace_Click(object sender, RoutedEventArgs e)
        {
            _mainWindow.ReplaceNext(
                TbFind.Text.Trim(),
                CbCaseSensitive.IsChecked != null && CbCaseSensitive.IsChecked.Value,
                TbReplace.Text.Trim());

        }

        private void BtnReplaceAll_Click(object sender, RoutedEventArgs e)
        {
            _mainWindow.ReplaceAll(
                TbFind.Text.Trim(),
                CbCaseSensitive.IsChecked != null && CbCaseSensitive.IsChecked.Value,
                TbReplace.Text.Trim());
        }

        private void SOpacity_ValueChanged(object sender, RoutedPropertyChangedEventArgs<double> e)
        {
            this.Opacity = e.NewValue;
        }

        private void BtnClose_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
