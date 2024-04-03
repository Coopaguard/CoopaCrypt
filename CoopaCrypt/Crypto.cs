using System;
using System.IO;
using System.Security.Cryptography;
using System.Security.Policy;
using System.Windows.Resources;

namespace CoopaCrypt
{
    public class Crypto
    {
        public enum EAction
        {
            Crypt,
            DeCrypt
        }
        private static byte[] GetBytes()
        {
            Uri uri = new Uri("/Assets/chiffrement.png", UriKind.Relative);
            StreamResourceInfo info = System.Windows.Application.GetResourceStream(uri);
            using var memoryStream = new MemoryStream();
            info.Stream.CopyTo(memoryStream);
            return SHA256.HashData(memoryStream.ToArray());
        }        

        public static void Crypt(string filePath, string content, string pwd)
        {
            var bytes = EncryptStringToBytes_Aes(content, pwd.HashString(), GetBytes());
            File.WriteAllBytes(filePath, bytes);
        }

        public static string Decrypt(string filePath, string pwd)
        {
            var bytes = File.ReadAllBytes(filePath);
            return DecryptStringFromBytes_Aes(bytes, pwd.HashString(), GetBytes());
        }

        static byte[] EncryptStringToBytes_Aes(string content, byte[] Key, byte[] IV)
        {
            // Check arguments.
            if (content == null || content.Length <= 0)
                throw new ArgumentNullException(nameof(content));
            if (Key == null || Key.Length <= 0)
                throw new ArgumentNullException(nameof(Key));
            if (IV == null || IV.Length <= 0)
                throw new ArgumentNullException(nameof(IV));
            byte[] encrypted;

            // Create an Aes object
            // with the specified key and IV.
            using (Aes aesAlg = Aes.Create())
            {
                aesAlg.Mode = CipherMode.CBC;
                aesAlg.Padding = PaddingMode.PKCS7;
                aesAlg.Key = Key;
                aesAlg.IV = IV[8..24];

                // Create an encryptor to perform the stream transform.
                ICryptoTransform encryptor = aesAlg.CreateEncryptor(aesAlg.Key, aesAlg.IV);

                // Create the streams used for encryption.
                using MemoryStream msEncrypt = new();
                using CryptoStream csEncrypt = new(msEncrypt, encryptor, CryptoStreamMode.Write);
                using (StreamWriter swEncrypt = new(csEncrypt))
                {
                    //Write all data to the stream.
                    swEncrypt.Write(content);
                }
                encrypted = msEncrypt.ToArray();
            }

            // Return the encrypted bytes from the memory stream.
            return encrypted;
        }

        static string DecryptStringFromBytes_Aes(byte[] cipherText, byte[] Key, byte[] IV)
        {
            // Check arguments.
            if (cipherText == null || cipherText.Length <= 0)
                throw new ArgumentNullException(nameof(cipherText));
            if (Key == null || Key.Length <= 0)
                throw new ArgumentNullException(nameof(Key));
            if (IV == null || IV.Length <= 0)
                throw new ArgumentNullException(nameof(IV));

            // Create an Aes object
            // with the specified key and IV.
            using Aes aesAlg = Aes.Create();
            aesAlg.Mode = CipherMode.CBC;
            aesAlg.Padding = PaddingMode.PKCS7;
            aesAlg.Key = Key;
            aesAlg.IV = IV[8..24];

            // Create a decryptor to perform the stream transform.
            ICryptoTransform decryptor = aesAlg.CreateDecryptor(aesAlg.Key, aesAlg.IV);

            // Create the streams used for decryption.
            using MemoryStream msDecrypt = new(cipherText);
            using CryptoStream csDecrypt = new(msDecrypt, decryptor, CryptoStreamMode.Read);
            using StreamReader srDecrypt = new(csDecrypt);

            // Read the decrypted bytes from the decrypting stream
            // and place them in a string.
            return srDecrypt.ReadToEnd();
        }

    }
}
