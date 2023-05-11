```
# 添加
iptables -t nat -A OUTPUT -p tcp -m multiport --dports 80,443 -j REDIRECT --to-port 9999
# 删除
iptables -t nat -D OUTPUT -p tcp -m multiport --dports 80,443 -j REDIRECT --to-port 9999
```
