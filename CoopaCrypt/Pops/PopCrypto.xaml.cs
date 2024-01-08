using System;
using System.Collections.Generic;
using System.Linq;
using System.Security;
using System.Security.Cryptography.X509Certificates;
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
    /// Interaction logic for PopCrypto.xaml
    /// </summary>
    public partial class PopCrypto : Window
    {

        public string? Pwd;

        public PopCrypto()
        {
            InitializeComponent();

            this.LoadPosition("PopMdp");

            this.TbPwd.Focus();
        }

        private void cancelBtn_Click(object sender, RoutedEventArgs e)
        {
            this.Close();
        }

        private void OkButton_Click(object sender, RoutedEventArgs e)
        {
            this.Pwd = TbPwd.Password;
            this.Close();
        }

        private void TbPwd_KeyDown(object sender, KeyEventArgs e)
        {
            if(e.Key == Key.Enter)
            {
                this.OkButton_Click(sender, e);
            }

            if (e.Key == Key.Escape)
            {
                this.cancelBtn_Click(sender, e);
            }
        }

        private void Window_Closing(object sender, System.ComponentModel.CancelEventArgs e)
        {
            this.SavePosition("PopMdp");
        }
    }
}
