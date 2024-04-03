using System.Windows;
using System.Windows.Input;

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
            if (e.Key == Key.Enter)
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
